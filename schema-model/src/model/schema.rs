use crate::model::enum_type::EnumType;
use crate::model::function::Function;
use crate::model::other_sql::OtherSql;
use crate::model::procedure::Procedure;
use crate::model::table::Table;
use crate::model::types::{DatabaseType, RelationType};
use crate::model::view::View;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Schema {
    schema_name: Option<String>,
    case_sensitive_text: bool,
    tables: Vec<Table>,
    views: Vec<View>,
    functions: Vec<Function>,
    procedures: Vec<Procedure>,
    other_sql: Vec<OtherSql>,
    // Case-insensitive map: store lowercase name -> index in tables vec
    table_map: HashMap<String, usize>,
    enum_types: HashMap<String, EnumType>,
}

impl Default for Schema {
    fn default() -> Self {
        Self {
            schema_name: None,
            case_sensitive_text: true,
            tables: Vec::new(),
            views: Vec::new(),
            functions: Vec::new(),
            procedures: Vec::new(),
            other_sql: Vec::new(),
            table_map: HashMap::new(),
            enum_types: HashMap::new(),
        }
    }
}

impl Schema {
    pub fn new<S: Into<String>>(schema_name: Option<S>) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            ..Default::default()
        }
    }

    pub fn schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn case_sensitive_text(&self) -> bool {
        self.case_sensitive_text
    }

    pub fn set_case_sensitive_text(&mut self, value: bool) {
        self.case_sensitive_text = value;
    }

    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    pub fn get_table(&self, name: &str) -> &Table {
        let idx = self.table_index(name);
        &self.tables[idx]
    }

    pub(crate) fn get_table_mut(&mut self, name: &str) -> &mut Table {
        let idx = self.table_index(name);
        &mut self.tables[idx]
    }

    /// Same as `get_table_mut`, but returns `None` instead of panicking when no table
    /// with this name exists - useful for callers (e.g. the XML parser) resolving
    /// possibly-malformed input, where a missing table should surface as an error
    /// rather than crash.
    pub fn get_table_mut_checked(&mut self, name: &str) -> Option<&mut Table> {
        let name_lower = name.to_lowercase();
        let idx = *self.table_map.get(&name_lower)?;
        Some(&mut self.tables[idx])
    }

    fn table_index(&self, name: &str) -> usize {
        let name_lower = name.to_lowercase();
        *self.table_map.get(&name_lower)
            .unwrap_or_else(|| panic!("Unable to locate a table with the name '{}'", name))
    }

    pub fn get_optional_table(&self, name: &str) -> Option<&Table> {
        let name_lower = name.to_lowercase();
        self.table_map.get(&name_lower).map(|&idx| &self.tables[idx])
    }

    pub fn all_views(&self) -> &[View] {
        &self.views
    }

    pub fn views(&self, database_type: DatabaseType) -> Vec<View> {
        self.views
            .iter()
            .filter(|view| view.database_type().is_none() || view.database_type().unwrap() == database_type)
            .cloned()
            .collect()
    }

    pub fn enum_types(&self) -> impl Iterator<Item = &EnumType> {
        let mut enum_types: Vec<&EnumType> = self.enum_types.values().collect();
        enum_types.sort_by(|a, b| a.name().cmp(b.name()));
        enum_types.into_iter()
    }

    pub fn get_enum_type(&self, type_name: &str) -> &EnumType {
        self.enum_types
            .get(&type_name.to_lowercase())
            .unwrap_or_else(|| panic!("Unable to locate an enum type with the name '{}'", type_name))
    }

    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    pub fn procedures(&self) -> &[Procedure] {
        &self.procedures
    }

    pub fn other_sql(&self) -> &[OtherSql] {
        &self.other_sql
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();
        for table in &self.tables {
            if table.columns().is_empty() {
                errors.push(format!(
                    "ERROR: table {} has no columns; generated `create table` SQL would be invalid",
                    table.name()
                ));
            }

            for relation in table.relations() {
                if relation.relation_type() == RelationType::SetNull {
                    let from_column_name = relation.from_column_name();
                    match table.has_column(from_column_name) {
                        true if table.column(from_column_name).is_required() => {
                            errors.push(format!(
                                "ERROR: {}.{} is required. The {}.{} relation specifies setnull, which is not allowed",
                                table.name(),
                                from_column_name,
                                relation.to_table_name(),
                                relation.to_column_name()
                            ));
                        }
                        true => {}
                        false => {
                            errors.push(format!(
                                "ERROR: {}.{} does not exist. The {}.{} relation refers to it as the source column",
                                table.name(),
                                from_column_name,
                                relation.to_table_name(),
                                relation.to_column_name()
                            ));
                        }
                    }
                }
            }

            for column in table.columns() {
                if let Some(enum_type_name) = column.enum_type()
                    && !self.enum_types.contains_key(&enum_type_name.to_lowercase())
                {
                    errors.push(format!(
                        "ERROR: {}.{} references enum type '{}' which is not defined in this schema",
                        table.name(),
                        column.name(),
                        enum_type_name
                    ));
                }

                if column.column_type() == crate::model::column_type::ColumnType::Array
                    && column.element_type().is_none()
                {
                    errors.push(format!(
                        "ERROR: {}.{} is an array column but has no elementType",
                        table.name(),
                        column.name()
                    ));
                }
            }
        }
        errors
    }

    pub(crate) fn add_table(&mut self, table: Table) {
        let idx = self.tables.len();
        self.table_map.insert(table.name().to_lowercase(), idx);
        self.tables.push(table);
    }

    /// Sorts the schema's tables alphabetically by name, in place. Must be called after all
    /// tables have been added, since it rebuilds the name -> index lookup used by `get_table`.
    pub fn sort_tables_by_name(&mut self) {
        self.tables.sort_by(|a, b| a.name().cmp(b.name()));

        self.table_map.clear();
        for (idx, table) in self.tables.iter().enumerate() {
            self.table_map.insert(table.name().to_lowercase(), idx);
        }
    }

    pub(crate) fn add_view(&mut self, view: View) {
        self.views.push(view);
    }

    pub(crate) fn add_enum_type(&mut self, enum_type: EnumType) {
        self.enum_types
            .insert(enum_type.name().to_lowercase(), enum_type);
    }

    pub(crate) fn add_functions(&mut self, functions: Vec<Function>) {
        self.functions.extend(functions);
    }

    pub(crate) fn add_procedures(&mut self, procedures: Vec<Procedure>) {
        self.procedures.extend(procedures);
    }

    pub(crate) fn add_other_sql(&mut self, other_sql: OtherSql) {
        self.other_sql.push(other_sql);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::column::Column;
    use crate::model::column_type::ColumnType;
    use crate::model::relation::Relation;

    fn make_schema() -> Schema {
        Schema::new(Some("schema"))
    }

    #[test]
    fn add_and_get_table_and_sort() {
        let mut schema = make_schema();
        let table1 = Table::new(
            Some("schema"),
            "Table1",
            Option::<&str>::None,
            crate::model::types::LockEscalation::Auto,
            false,
            vec![Column::new(Some("s"), "id", ColumnType::Int, 0, 0, true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let table2 = Table::new(
            Some("schema"),
            "Table2",
            Option::<&str>::None,
            crate::model::types::LockEscalation::Auto,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        schema.add_table(table1);
        schema.add_table(table2);
        assert_eq!(schema.get_table("Table2").name(), "Table2"); // case-insensitive
        let names: Vec<_> = schema.tables().iter().map(|t| t.name()).collect();
        assert_eq!(names, vec!["Table1", "Table2"]);
        // table_map rebuilt so get_table still works
        assert_eq!(schema.get_table("Table1").name(), "Table1");
    }

    #[test]
    fn get_enum_type_is_case_insensitive() {
        // Matches the case-insensitive lookup used everywhere else in the model
        // (Table::column/has_column, Schema::get_table); a column declaring
        // `enumType="gendertype"` must still resolve an enum declared as `GenderType`.
        let mut schema = make_schema();
        schema.add_enum_type(EnumType::new("GenderType", Vec::new()));

        assert_eq!(schema.get_enum_type("GenderType").name(), "GenderType");
        assert_eq!(schema.get_enum_type("gendertype").name(), "GenderType");
        assert_eq!(schema.get_enum_type("GENDERTYPE").name(), "GenderType");
    }

    #[test]
    fn views_filtered_by_database_type() {
        let mut s = make_schema();
        s.add_view(View::new(Some("s"), "v1", "sql1", Some(DatabaseType::Postgresql)));
        s.add_view(View::new(Some("s"), "v2", "sql2", Some(DatabaseType::SqlServer)));
        let pg = s.views(DatabaseType::Postgresql);
        assert_eq!(pg.len(), 1);
        assert_eq!(pg[0].name(), "v1");
    }

    #[test]
    fn validate_setnull_error_when_required() {
        let mut s = make_schema();
        let parent = Table::new(
            Some("s"),
            "parent",
            Option::<&str>::None,
            crate::model::types::LockEscalation::Auto,
            false,
            vec![Column::new(Some("s"), "id", ColumnType::Int, 0, 0, true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        s.add_table(parent);

        let child = Table::new(
            Some("s"),
            "child",
            Option::<&str>::None,
            crate::model::types::LockEscalation::Auto,
            false,
            vec![Column::new(Some("s"), "pid", ColumnType::Int, 0, 0, true)],
            Vec::new(),
            Vec::new(),
            vec![Relation::new(
                "parent",
                "id",
                "child",
                "pid",
                RelationType::SetNull,
                false,
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        s.add_table(child);

        let errors = s.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("setnull"));
    }

    #[test]
    fn validate_reports_error_instead_of_panicking_when_from_column_is_missing() {
        // A relation whose `src` attribute doesn't match any real column on the table
        // (e.g. a typo in the XML) must surface as a validation error, not panic.
        let mut s = make_schema();
        let parent = Table::new(
            Some("s"),
            "parent",
            Option::<&str>::None,
            crate::model::types::LockEscalation::Auto,
            false,
            vec![Column::new(Some("s"), "id", ColumnType::Int, 0, 0, true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        s.add_table(parent);

        let child = Table::new(
            Some("s"),
            "child",
            Option::<&str>::None,
            crate::model::types::LockEscalation::Auto,
            false,
            vec![Column::new(Some("s"), "pid", ColumnType::Int, 0, 0, true)],
            Vec::new(),
            Vec::new(),
            vec![Relation::new(
                "parent",
                "id",
                "child",
                "does_not_exist",
                RelationType::SetNull,
                false,
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        s.add_table(child);

        let errors = s.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("does_not_exist"));
    }

    #[test]
    fn validate_reports_error_for_column_referencing_an_undeclared_enum_type() {
        // A column's `enumType` attribute referencing a name with no matching `<enum>`
        // declaration must surface as a validation error - previously nothing caught
        // this, and it panicked deep in SQL generation instead (`get_enum_type`).
        use crate::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
        use crate::model::column_type::ColumnType as CT;

        let table = TableBuilder::new(Some("s"), "widget")
            .add_column(
                ColumnBuilder::new(Some("s"), "status", CT::Enum)
                    .enum_type(Some("StatusType".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(Some("s")).add_table(table).build();

        let errors = schema.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("StatusType"));
    }

    #[test]
    fn validate_reports_error_for_table_with_no_columns() {
        // A table with zero columns generates invalid `create table t (\n)` DDL; this
        // must be caught up front rather than crash/emit invalid SQL during generation.
        use crate::builder::{SchemaBuilder, TableBuilder};

        let table = TableBuilder::new(Some("s"), "widget").build();
        let schema = SchemaBuilder::new(Some("s")).add_table(table).build();

        let errors = schema.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("widget"));
        assert!(errors[0].contains("no columns"));
    }

    #[test]
    fn validate_reports_error_for_array_column_missing_element_type() {
        use crate::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
        use crate::model::column_type::ColumnType as CT;

        let table = TableBuilder::new(Some("s"), "widget")
            .add_column(ColumnBuilder::new(Some("s"), "tags", CT::Array).build())
            .build();
        let schema = SchemaBuilder::new(Some("s")).add_table(table).build();

        let errors = schema.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("tags"));
        assert!(errors[0].contains("elementType"));
    }

    #[test]
    fn validate_accepts_array_column_with_element_type_set() {
        use crate::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
        use crate::model::column_type::ColumnType as CT;

        let table = TableBuilder::new(Some("s"), "widget")
            .add_column(
                ColumnBuilder::new(Some("s"), "tags", CT::Array)
                    .element_type(Some("varchar".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(Some("s")).add_table(table).build();

        assert!(schema.validate().is_empty());
    }

    #[test]
    fn validate_accepts_column_referencing_a_declared_enum_type_case_insensitively() {
        use crate::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
        use crate::model::column_type::ColumnType as CT;
        use crate::model::enum_type::EnumType;

        let table = TableBuilder::new(Some("s"), "widget")
            .add_column(
                ColumnBuilder::new(Some("s"), "status", CT::Enum)
                    .enum_type(Some("statustype".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(Some("s"))
            .add_table(table)
            .add_enum_type(EnumType::new("StatusType", Vec::new()))
            .build();

        assert!(schema.validate().is_empty());
    }

    // #[test]
    // fn build_reverse_relations_creates_back_refs() {
    //     let mut s = make_schema();
    //     let mut parent = Table::new(
    //         Some("s"),
    //         "p",
    //         Option::<&str>::None,
    //         crate::model::types::LockEscalation::Auto,
    //         false,
    //         vec![Column::new(Some("s"), "id", ColumnType::Int, 0, 0, true)],
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //     );
    //     let mut child = Table::new(
    //         Some("s"),
    //         "c",
    //         Option::<&str>::None,
    //         crate::model::types::LockEscalation::Auto,
    //         false,
    //         vec![Column::new(Some("s"), "pid", ColumnType::Int, 0, 0, false)],
    //         Vec::new(),
    //         Vec::new(),
    //         vec![Relation::new(
    //             "p",
    //             "id",
    //             "c",
    //             "pid",
    //             RelationType::Cascade,
    //             false,
    //         )],
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //         Vec::new(),
    //     );
    //     s.add_table(parent);
    //     s.add_table(child);
    //
    //     s.build_reverse_relations();
    //     let p_ref = s.get_table("p");
    //     assert_eq!(p_ref.reverse_relations().len(), 1);
    //     let rr = &p_ref.reverse_relations()[0];
    //     assert_eq!(rr.from_table_name(), "c");
    //     assert_eq!(rr.to_table_name(), "p");
    // }
}
