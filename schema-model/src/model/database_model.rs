use crate::model::enum_type::EnumType;
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::types::{BooleanMode, ForeignKeyMode};

/// Schema names compare case-insensitively, like table and column names; `None` is the
/// default schema and only matches `None`.
fn schema_names_match(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
        _ => false,
    }
}

/// Splits `table` or `schema.table` into `(schema, table)`. Anything with more than one
/// dot is rejected rather than guessed at (it used to silently resolve as a table
/// literally named `a.b.c`).
fn split_qualified_name(qualified_name: &str) -> Result<(Option<&str>, &str), String> {
    let parts: Vec<&str> = qualified_name.split('.').collect();
    match parts.as_slice() {
        [table] => Ok((None, table)),
        [schema, table] => Ok((Some(schema), table)),
        _ => Err(format!(
            "'{}' is not a valid table reference; expected 'table' or 'schema.table'",
            qualified_name
        )),
    }
}

#[derive(Debug, Default, Clone)]
pub struct DatabaseModel {
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    schemas: Vec<Schema>,
}

impl DatabaseModel {
    pub fn new(boolean_mode: BooleanMode,
               foreign_key_mode: ForeignKeyMode,
               schemas: Vec<Schema>) -> Self {
        Self {
            boolean_mode,
            foreign_key_mode,
            schemas,
        }
    }

    pub fn foreign_key_mode(&self) -> ForeignKeyMode {
        self.foreign_key_mode
    }

    pub fn boolean_mode(&self) -> BooleanMode {
        self.boolean_mode
    }

    pub fn schemas(&self) -> &Vec<Schema> {
        &self.schemas
    }

    pub fn default_schema(&self) -> &Schema {
        self.schemas
            .iter()
            .find(|s| s.schema_name().is_none())
            .expect("Default schema not found")
    }

    pub fn default_schema_mut(&mut self) -> &mut Schema {
        self.schemas
            .iter_mut()
            .find(|s| s.schema_name().is_none())
            .expect("Default schema not found")
    }

    pub fn find_schema(&self, schema_name: Option<&str>) -> &Schema {
        if schema_name.is_none() {
            return self.default_schema();
        }

        self.schemas
            .iter()
            .find(|s| schema_names_match(s.schema_name(), schema_name))
            .unwrap_or_else(|| panic!("Schema '{}' not found", schema_name.unwrap()))
    }

    pub(crate) fn find_schema_mut(&mut self, schema_name: Option<&str>) -> &mut Schema {
        if schema_name.is_none() {
            return self.default_schema_mut();
        }

        self.schemas
            .iter_mut()
            .find(|s| schema_names_match(s.schema_name(), schema_name))
            .unwrap_or_else(|| panic!("Schema '{}' not found", schema_name.unwrap()))
    }

    /// Resolves the enum a column in `schema_name` refers to: the column's own schema
    /// first, then the database-level (default-schema) `<enum>` declarations, so a
    /// column inside `<schema name="app">` can use a shared top-level enum (M5). An enum
    /// declared in the column's own schema shadows a same-named database-level one.
    ///
    /// Returns the schema that owns the enum (`None` = the default schema) alongside it,
    /// since a generator that emits a schema-qualified type name needs the owner, not
    /// the referring column's schema (M4).
    pub fn resolve_enum_type(&self, schema_name: Option<&str>, enum_type: &str) -> Option<(Option<&str>, &EnumType)> {
        let in_schema = |name: Option<&str>| {
            self.schemas
                .iter()
                .find(|s| schema_names_match(s.schema_name(), name))
                .and_then(|s| s.get_optional_enum_type(enum_type).map(|found| (s.schema_name(), found)))
        };
        if schema_name.is_some()
            && let Some(found) = in_schema(schema_name)
        {
            return Some(found);
        }
        in_schema(None)
    }

    pub fn find_enum_type(&self, schema_name: Option<&str>, enum_type: &str) -> &EnumType {
        self.resolve_enum_type(schema_name, enum_type)
            .map(|(_, found)| found)
            .unwrap_or_else(|| panic!("Unable to locate an enum type with the name '{}'", enum_type))
    }

    pub fn all_tables(&self) -> Vec<&Table> {
        self.schemas
            .iter()
            .flat_map(|s| s.tables())
            .collect()
    }

    pub fn find_table(&self, schema_name: Option<&str>, table_name: &str) -> &Table {
        let schema = self.find_schema(schema_name);
        schema.get_table(table_name)
    }

    pub fn find_table_by_qualified_name(&self, qualified_name: &str) -> &Table {
        let (schema_name, table_name) = split_qualified_name(qualified_name)
            .unwrap_or_else(|e| panic!("{}", e));

        self.find_table(schema_name, table_name)
    }

    /// Same as `find_table_by_qualified_name`, but returns `None` instead of panicking
    /// when the schema or table doesn't exist - used by `validate()` to check relation
    /// targets without crashing on a dangling one.
    pub fn find_table_by_qualified_name_checked(&self, qualified_name: &str) -> Option<&Table> {
        let (schema_name, table_name) = split_qualified_name(qualified_name).ok()?;

        self.schemas
            .iter()
            .find(|s| schema_names_match(s.schema_name(), schema_name))
            .and_then(|s| s.get_optional_table(table_name))
    }

    /// Checks model-wide invariants that individual `Schema::validate()` calls can't see
    /// (a relation's target table may live in a different schema than the one declaring
    /// it). Combined with each schema's own `validate()`, this is meant to be run once
    /// after parsing/building a model and before handing it to SQL generation or
    /// migration - several generator code paths (`find_enum_type`, `find_table_by_qualified_name`,
    /// ...) panic on a reference that doesn't resolve, on the assumption the model was
    /// already validated.
    pub fn validate(&self) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();

        for schema in &self.schemas {
            errors.extend(schema.validate());
        }

        for schema in &self.schemas {
            for table in schema.tables() {
                for column in table.columns() {
                    if let Some(enum_type_name) = column.enum_type()
                        && self.resolve_enum_type(schema.schema_name(), enum_type_name).is_none()
                    {
                        errors.push(format!(
                            "ERROR: {}.{} references enum type '{}' which is not defined in {}",
                            table.name(),
                            column.name(),
                            enum_type_name,
                            match schema.schema_name() {
                                Some(name) => format!("schema '{}' or at the database level", name),
                                None => "this database".to_string(),
                            }
                        ));
                    }
                }
            }
        }

        for table in self.all_tables() {
            for relation in table.relations() {
                match split_qualified_name(relation.to_table_name()) {
                    Err(e) => errors.push(format!(
                        "ERROR: {}.{} has a relation with an invalid target: {}",
                        table.name(),
                        relation.from_column_name(),
                        e
                    )),
                    Ok(_) => match self.find_table_by_qualified_name_checked(relation.to_table_name()) {
                        None => errors.push(format!(
                            "ERROR: {}.{} has a relation to '{}' which does not exist",
                            table.name(),
                            relation.from_column_name(),
                            relation.to_table_name()
                        )),
                        Some(target) => {
                            for (from_column, to_column) in relation.column_pairs() {
                                if !target.has_column(to_column) {
                                    errors.push(format!(
                                        "ERROR: {}.{} has a relation to {}.{}, but {} has no column '{}'",
                                        table.name(),
                                        from_column,
                                        relation.to_table_name(),
                                        to_column,
                                        target.name(),
                                        to_column
                                    ));
                                }
                            }
                        }
                    },
                }
            }
        }

        errors
    }

    pub fn find_table_mut(&mut self, schema_name: Option<&str>, table_name: &str) -> &mut Table {
        let schema = self.find_schema_mut(schema_name);
        schema.get_table_mut(table_name)
    }

    /// Same as `find_table_mut`, but returns `None` instead of panicking when the schema
    /// or table doesn't exist - useful for callers (e.g. the XML parser) resolving
    /// possibly-malformed input, where a dangling reference should surface as an error
    /// rather than crash.
    pub fn find_table_mut_checked(&mut self, schema_name: Option<&str>, table_name: &str) -> Option<&mut Table> {
        let schema = self.schemas.iter_mut().find(|s| schema_names_match(s.schema_name(), schema_name))?;
        schema.get_table_mut_checked(table_name)
    }

    pub fn sort_tables_by_name(&mut self) {
        for schema in self.schemas.iter_mut() {
            schema.sort_tables_by_name();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use crate::model::column_type::ColumnType;
    use crate::model::relation::Relation;
    use crate::model::table::Table;
    use crate::model::types::RelationType;

    #[test]
    fn validate_reports_error_for_a_relation_whose_target_table_does_not_exist() {
        // A relation whose target table doesn't exist anywhere in the model (typo'd
        // name, or a schema that was never declared) must surface as a validation
        // error - several generator code paths panic on this instead if it's not
        // caught up front.
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("Regoin", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(child).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        let errors = model.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Regoin"));
    }

    #[test]
    fn validate_accepts_a_relation_whose_target_table_exists_in_a_different_schema() {
        let parent = TableBuilder::new(Some("app"), "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let parent_schema = SchemaBuilder::new(Some("app")).add_table(parent).build();

        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("app.parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let default_schema = SchemaBuilder::new(None::<&str>).add_table(child).build();

        let model = DatabaseModel::new(
            BooleanMode::Native,
            ForeignKeyMode::Relations,
            vec![default_schema, parent_schema],
        );

        assert!(model.validate().is_empty());
    }

    fn enum_column_table(schema: Option<&str>, enum_name: &str) -> Table {
        TableBuilder::new(schema, "widget")
            .add_column(
                ColumnBuilder::new(schema, "status", ColumnType::Enum)
                    .enum_type(Some(enum_name.to_string()))
                    .build(),
            )
            .build()
    }

    fn enum_named(name: &str) -> crate::model::enum_type::EnumType {
        crate::model::enum_type::EnumType::new(name, Vec::new())
    }

    #[test]
    fn validate_reports_error_for_column_referencing_an_undeclared_enum_type() {
        // Previously nothing caught this and it panicked deep in SQL generation.
        let schema = SchemaBuilder::new(Some("s")).add_table(enum_column_table(Some("s"), "StatusType")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        let errors = model.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("StatusType"));
    }

    #[test]
    fn validate_accepts_a_database_level_enum_used_from_a_named_schema() {
        // M5: a top-level `<enum>` lives in the default schema; a column inside
        // `<schema name="app">` must be able to reference it.
        let default_schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_named("StatusType")).build();
        let app = SchemaBuilder::new(Some("app")).add_table(enum_column_table(Some("app"), "statustype")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![default_schema, app]);

        assert!(model.validate().is_empty());
    }

    #[test]
    fn resolve_enum_type_prefers_own_schema_and_reports_the_owner() {
        let default_schema = SchemaBuilder::new(None::<&str>)
            .add_enum_type(enum_named("StatusType"))
            .add_enum_type(enum_named("SharedType"))
            .build();
        let app = SchemaBuilder::new(Some("app")).add_enum_type(enum_named("StatusType")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![default_schema, app]);

        assert_eq!(model.resolve_enum_type(Some("app"), "StatusType").unwrap().0, Some("app"));
        assert_eq!(model.resolve_enum_type(Some("app"), "SharedType").unwrap().0, None);
        assert_eq!(model.resolve_enum_type(None, "StatusType").unwrap().0, None);
        assert!(model.resolve_enum_type(None, "Missing").is_none());
    }

    fn id_table(schema: Option<&str>, name: &str) -> Table {
        TableBuilder::new(schema, name)
            .add_column(ColumnBuilder::new(schema, "id", ColumnType::Sequence).required(true).build())
            .build()
    }

    #[test]
    fn validate_reports_error_for_a_relation_whose_target_column_does_not_exist() {
        // M11: only the target *table* used to be checked.
        let parent = id_table(None, "parent");
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("parent", "idd", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(parent).add_table(child).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        let errors = model.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("idd"));
    }

    #[test]
    fn find_schema_is_case_insensitive() {
        // M12: table and column lookups are case-insensitive; schema lookup must match.
        let app = SchemaBuilder::new(Some("App")).add_table(id_table(Some("App"), "parent")).build();
        let default_schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![default_schema, app]);

        assert_eq!(model.find_schema(Some("app")).schema_name(), Some("App"));
        assert_eq!(model.find_table_by_qualified_name("APP.Parent").name(), "parent");
        assert!(model.find_table_by_qualified_name_checked("app.parent").is_some());
    }

    #[test]
    fn three_part_qualified_name_is_rejected_not_treated_as_unqualified() {
        // M12: `db.schema.table` used to resolve a table literally named `db.schema.table`.
        let model = DatabaseModel::new(
            BooleanMode::Native,
            ForeignKeyMode::Relations,
            vec![SchemaBuilder::new(None::<&str>).add_table(id_table(None, "a.b.c")).build()],
        );

        assert!(model.find_table_by_qualified_name_checked("a.b.c").is_none());
    }
}
