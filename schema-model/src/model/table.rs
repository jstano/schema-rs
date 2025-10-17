use std::fmt;
use crate::model::column::Column;
use crate::model::relation::Relation;
use crate::model::types::{BooleanMode, KeyType, TableOption, LockEscalation};
use crate::model::trigger::Trigger;
use crate::model::constraint::Constraint;
use crate::model::initial_data::InitialData;
use crate::model::aggregation::Aggregation;
use crate::model::column_type::ColumnType;
use crate::model::key::Key;

#[derive(Debug, Clone)]
pub struct Table {
    schema_name: String,
    name: String,
    export_date_column: Option<String>,
    lock_escalation: LockEscalation,
    no_export: bool,
    columns: Vec<Column>,
    keys: Vec<Key>,
    indexes: Vec<Key>,
    relations: Vec<Relation>,
    reverse_relations: Vec<Relation>,
    triggers: Vec<Trigger>,
    constraints: Vec<Constraint>,
    initial_data: Vec<InitialData>,
    options: Vec<TableOption>,
    aggregations: Vec<Aggregation>,
}

impl Table {
    #[allow(clippy::too_many_arguments)]
    pub fn new<S: Into<String>>(
        schema_name: S,
        name: S,
        export_date_column: Option<S>,
        lock_escalation: LockEscalation,
        no_export: bool,
    ) -> Self {
        Self {
            schema_name: schema_name.into(),
            name: name.into(),
            export_date_column: export_date_column.map(|s| s.into()),
            lock_escalation,
            no_export,
            columns: Vec::new(),
            keys: Vec::new(),
            indexes: Vec::new(),
            relations: Vec::new(),
            reverse_relations: Vec::new(),
            triggers: Vec::new(),
            constraints: Vec::new(),
            initial_data: Vec::new(),
            options: Vec::new(),
            aggregations: Vec::new(),
        }
    }

    pub fn schema_name(&self) -> &str { &self.schema_name }
    pub fn name(&self) -> &str { &self.name }
    pub fn export_date_column(&self) -> Option<&str> { self.export_date_column.as_deref() }
    pub fn lock_escalation(&self) -> LockEscalation { self.lock_escalation }
    pub fn is_no_export(&self) -> bool { self.no_export }

    pub fn columns(&self) -> &Vec<Column> { &self.columns }
    pub fn columns_mut(&mut self) -> &mut Vec<Column> { &mut self.columns }

    pub fn keys(&self) -> &Vec<Key> { &self.keys }
    pub fn keys_mut(&mut self) -> &mut Vec<Key> { &mut self.keys }

    pub fn indexes(&self) -> &Vec<Key> { &self.indexes }
    pub fn indexes_mut(&mut self) -> &mut Vec<Key> { &mut self.indexes }

    pub fn relations(&self) -> &Vec<Relation> { &self.relations }
    pub fn relations_mut(&mut self) -> &mut Vec<Relation> { &mut self.relations }

    pub fn reverse_relations(&self) -> &Vec<Relation> { &self.reverse_relations }
    pub fn reverse_relations_mut(&mut self) -> &mut Vec<Relation> { &mut self.reverse_relations }

    pub fn triggers(&self) -> &Vec<Trigger> { &self.triggers }
    pub fn triggers_mut(&mut self) -> &mut Vec<Trigger> { &mut self.triggers }

    pub fn constraints(&self) -> &Vec<Constraint> { &self.constraints }
    pub fn constraints_mut(&mut self) -> &mut Vec<Constraint> { &mut self.constraints }

    pub fn initial_data(&self) -> &Vec<InitialData> { &self.initial_data }
    pub fn initial_data_mut(&mut self) -> &mut Vec<InitialData> { &mut self.initial_data }

    pub fn options(&self) -> &Vec<TableOption> { &self.options }
    pub fn options_mut(&mut self) -> &mut Vec<TableOption> { &mut self.options }

    pub fn aggregations(&self) -> &Vec<Aggregation> { &self.aggregations }
    pub fn aggregations_mut(&mut self) -> &mut Vec<Aggregation> { &mut self.aggregations }

    pub fn column(&self, column_name: &str) -> &Column {
        let lower = column_name.to_lowercase();
        self.columns
            .iter()
            .find(|c| c.name().eq_ignore_ascii_case(&lower))
            .unwrap_or_else(|| panic!(
                "Unable to locate a column with the name '{}' in table '{}'",
                column_name, self.name
            ))
    }

    pub fn primary_key(&self) -> Option<&Key> {
        self.keys.iter().find(|k| k.r#type() == KeyType::Primary)
    }

    pub fn has_column(&self, column_name: &str) -> bool {
        let lower = column_name.to_lowercase();
        self.columns.iter().any(|c| c.name().eq_ignore_ascii_case(&lower))
    }

    pub fn identity_column(&self) -> Option<&Column> {
        self.columns.iter().find(|c| {
            let t = c.column_type();
            matches!(t, ColumnType::Sequence | ColumnType::LongSequence)
        })
    }

    pub fn primary_key_columns(&self) -> Option<Vec<String>> {
        self.primary_key().map(|k| k.columns().iter().map(|kc| kc.name().to_string()).collect())
    }

    pub fn has_option(&self, option: TableOption) -> bool {
        self.options.iter().any(|o| *o == option)
    }

    pub fn has_column_constraints(&self, boolean_mode: BooleanMode) -> bool {
        self.columns.iter().any(|c| c.needs_check_constraints(boolean_mode))
    }

    pub fn columns_with_check_constraints(&self, boolean_mode: BooleanMode) -> Vec<Column> {
        self.columns
            .iter()
            .filter(|c| c.needs_check_constraints(boolean_mode))
            .cloned()
            .collect()
    }

    pub fn column_relation(&self, column: &Column) -> Option<&Relation> {
        let name = column.name();
        self.relations
            .iter()
            .find(|r| r.from_column_name().eq_ignore_ascii_case(name))
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.name) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::{KeyType, TableOption, BooleanMode, LockEscalation};
    use crate::model::key::{Key, KeyColumn};

    fn sample_table() -> Table {
        Table::new("s", "t", Option::<&str>::None, LockEscalation::Auto, false)
    }

    #[test]
    fn constructor_and_basic_getters() {
        let t = sample_table();
        assert_eq!(t.schema_name(), "s");
        assert_eq!(t.name(), "t");
        assert_eq!(t.export_date_column(), None);
        assert_eq!(t.lock_escalation(), LockEscalation::Auto);
        assert!(!t.is_no_export());
        assert_eq!(format!("{}", t), "t");
    }

    #[test]
    fn columns_and_keys_behavior() {
        let mut t = sample_table();
        t.columns_mut().push(Column::new("id", ColumnType::Int, 0, 0, true));
        t.columns_mut().push(Column::new("name", ColumnType::Varchar, 255, 0, false));
        assert!(t.has_column("ID")); // case-insensitive
        assert_eq!(t.column("name").column_type(), ColumnType::Varchar);

        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        t.keys_mut().push(pk);
        assert_eq!(t.primary_key().unwrap().columns().len(), 1);
        assert_eq!(t.primary_key_columns().unwrap(), vec!["id".to_string()]);

        assert!(t.identity_column().is_none());
        t.columns_mut().push(Column::new("seq", ColumnType::Sequence, 0, 0, true));
        assert_eq!(t.identity_column().unwrap().name(), "seq");

        t.options_mut().push(TableOption::Compress);
        assert!(t.has_option(TableOption::Compress));

        assert!(t.has_column_constraints(BooleanMode::Native) == false);
        // Add a boolean; in YesNo mode constraints exist
        t.columns_mut().push(Column::new("b", ColumnType::Boolean, 0, 0, false));
        assert!(t.has_column_constraints(BooleanMode::YesNo));
        assert_eq!(t.columns_with_check_constraints(BooleanMode::YesNo).len(), 1);
    }

    #[test]
    fn relations_helpers() {
        let mut t = sample_table();
        t.columns_mut().push(Column::new("parent_id", ColumnType::Int, 0, 0, false));
        t.relations_mut().push(Relation::new(
            "parent",
            "id",
            "t",
            "parent_id",
            crate::model::types::RelationType::Cascade,
            false,
        ));
        let col = t.column("parent_id").clone();
        let r = t.column_relation(&col).unwrap();
        assert_eq!(r.to_table_name(), "parent");
    }
}
