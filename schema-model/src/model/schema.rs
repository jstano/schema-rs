use crate::model::enum_type::EnumType;
use crate::model::function::Function;
use crate::model::other_sql::OtherSql;
use crate::model::procedure::Procedure;
use crate::model::relation::Relation;
use crate::model::table::Table;
use crate::model::types::{DatabaseType, RelationType};
use crate::model::view::View;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Schema {
    schema_name: String,
    tables: Vec<Table>,
    views: Vec<View>,
    functions: Vec<Function>,
    procedures: Vec<Procedure>,
    other_sql: Vec<OtherSql>,
    // Case-insensitive map: store lowercase name -> index in tables vec
    table_map: HashMap<String, usize>,
    enum_types: HashMap<String, EnumType>,
}

impl Schema {
    pub fn new(schema_name: String) -> Self {
        Self {
            schema_name,
            ..Default::default()
        }
    }

    pub fn schema_name(&self) -> &str { &self.schema_name }

    pub fn tables(&self) -> &Vec<Table> { &self.tables }

    pub fn get_table(&self, name: &str) -> &Table {
        let key = name.to_lowercase();
        if let Some(&idx) = self.table_map.get(&key) {
            return &self.tables[idx];
        }
        panic!("Unable to locate a table with the name '{}'", name)
    }

    pub fn get_optional_table(&self, name: &str) -> Option<&Table> {
        let key = name.to_lowercase();
        self.table_map.get(&key).map(|&idx| &self.tables[idx])
    }

    pub fn views(&self, database_type: DatabaseType) -> Vec<View> {
        // With View holding a concrete DatabaseType, simply return those matching the requested type.
        self.views
            .iter()
            .filter(|v| v.database_type() == database_type)
            .cloned()
            .collect()
    }

    pub fn enum_types(&self) -> impl Iterator<Item = &EnumType> { self.enum_types.values() }

    pub fn get_enum_type(&self, type_name: &str) -> &EnumType {
        self.enum_types
            .get(type_name)
            .unwrap_or_else(|| panic!("Unable to locate an enum type with name '{}'", type_name))
    }

    pub fn add_table(&mut self, table: Table) {
        let idx = self.tables.len();
        self.table_map.insert(table.name().to_lowercase(), idx);
        self.tables.push(table);
    }

    pub fn add_view(&mut self, view: View) { self.views.push(view); }

    pub fn add_enum_type(&mut self, enum_type: EnumType) { self.enum_types.insert(enum_type.name().to_string(), enum_type); }

    pub fn add_functions(&mut self, functions: Vec<Function>) { self.functions.extend(functions); }
    pub fn functions(&self) -> Vec<Function> { self.functions.clone() }

    pub fn add_procedures(&mut self, procedures: Vec<Procedure>) { self.procedures.extend(procedures); }
    pub fn procedures(&self) -> Vec<Procedure> { self.procedures.clone() }

    pub fn add_other_sql(&mut self, other_sql: OtherSql) { self.other_sql.push(other_sql); }
    pub fn other_sql(&self) -> Vec<OtherSql> { self.other_sql.clone() }

    pub fn validate(&self) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();
        for table in &self.tables {
            for relation in table.relations() {
                if relation.relation_type() == RelationType::SetNull {
                    let from_table_name = relation.from_table_name().to_string();
                    let from_column_name = relation.from_column_name().to_string();
                    let from_table = self.get_table(&from_table_name);
                    if from_table.column(&from_column_name).is_required() {
                        errors.push(format!(
                            "ERROR: {}.{} is required. The {}.{} relation specifies setnull, which is not allowed",
                            from_table_name,
                            from_column_name,
                            relation.to_table_name(),
                            relation.to_column_name()
                        ));
                    }
                }
            }
        }
        errors
    }

    pub fn sort_tables_by_name(&mut self) {
        self.tables.sort_by_key(|t| t.name().to_string());
        // Rebuild the table_map to reflect new indices
        self.table_map.clear();
        for (idx, t) in self.tables.iter().enumerate() {
            self.table_map.insert(t.name().to_lowercase(), idx);
        }
    }

    pub fn build_reverse_relations(&mut self) {
        // We need mutable access to parent tables too, so handle indices carefully.
        // First, collect the relations to add per parent table to avoid multiple mutable borrows.
        let mut to_add: HashMap<usize, Vec<Relation>> = HashMap::new();
        for (child_idx, table) in self.tables.iter().enumerate() {
            if !table.relations().is_empty() {
                for relation in table.relations() {
                    let parent_name = relation.to_table_name();
                    if let Some(&parent_idx) = self.table_map.get(&parent_name.to_lowercase()) {
                        let reverse = Relation::new(
                            relation.to_table_name().to_string(),
                            relation.to_column_name().to_string(),
                            relation.from_table_name().to_string(),
                            relation.from_column_name().to_string(),
                            relation.relation_type(),
                            false,
                        );
                        to_add.entry(parent_idx).or_default().push(reverse);
                    } else {
                        // Parent not found; ignore or log in real implementation
                        let _ = child_idx; // keep variable used
                    }
                }
            }
        }
        for (idx, rels) in to_add {
            if let Some(parent) = self.tables.get_mut(idx) {
                parent.reverse_relations_mut().extend(rels);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::column::Column;
    use crate::model::column_type::ColumnType;

    fn make_schema() -> Schema { Schema::new("s".to_string()) }

    #[test]
    fn add_and_get_table_and_sort() {
        let mut s = make_schema();
        let mut t1 = Table::new("s", "B", Option::<&str>::None, crate::model::types::LockEscalation::Auto, false);
        let t2 = Table::new("s", "A", Option::<&str>::None, crate::model::types::LockEscalation::Auto, false);
        t1.columns_mut().push(Column::new("id", ColumnType::Int, 0, 0, true));
        s.add_table(t1);
        s.add_table(t2);
        assert_eq!(s.get_table("b").name(), "B"); // case-insensitive
        s.sort_tables_by_name();
        let names: Vec<_> = s.tables().iter().map(|t| t.name()).collect();
        assert_eq!(names, vec!["A", "B"]);
        // table_map rebuilt so get_table still works
        assert_eq!(s.get_table("A").name(), "A");
    }

    #[test]
    fn views_filtered_by_database_type() {
        let mut s = make_schema();
        s.add_view(View::new("s", "v1", "sql1", DatabaseType::Postgres));
        s.add_view(View::new("s", "v2", "sql2", DatabaseType::Mysql));
        let pg = s.views(DatabaseType::Postgres);
        assert_eq!(pg.len(), 1);
        assert_eq!(pg[0].name(), "v1");
    }

    #[test]
    fn validate_setnull_error_when_required() {
        let mut s = make_schema();
        let mut parent = Table::new("s", "parent", Option::<&str>::None, crate::model::types::LockEscalation::Auto, false);
        parent.columns_mut().push(Column::new("id", ColumnType::Int, 0, 0, true));
        s.add_table(parent);

        let mut child = Table::new("s", "child", Option::<&str>::None, crate::model::types::LockEscalation::Auto, false);
        child.columns_mut().push(Column::new("pid", ColumnType::Int, 0, 0, true));
        child.relations_mut().push(Relation::new(
            "parent", "id", "child", "pid", RelationType::SetNull, false
        ));
        s.add_table(child);

        let errors = s.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("setnull"));
    }

    #[test]
    fn build_reverse_relations_creates_back_refs() {
        let mut s = make_schema();
        let mut parent = Table::new("s", "p", Option::<&str>::None, crate::model::types::LockEscalation::Auto, false);
        parent.columns_mut().push(Column::new("id", ColumnType::Int, 0, 0, true));
        let mut child = Table::new("s", "c", Option::<&str>::None, crate::model::types::LockEscalation::Auto, false);
        child.columns_mut().push(Column::new("pid", ColumnType::Int, 0, 0, false));
        child.relations_mut().push(Relation::new("p", "id", "c", "pid", RelationType::Cascade, false));
        s.add_table(parent);
        s.add_table(child);

        s.build_reverse_relations();
        let p_ref = s.get_table("p");
        assert_eq!(p_ref.reverse_relations().len(), 1);
        let rr = &p_ref.reverse_relations()[0];
        assert_eq!(rr.from_table_name(), "c");
        assert_eq!(rr.to_table_name(), "p");
    }
}
