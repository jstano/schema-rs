use crate::model::aggregation::Aggregation;
use crate::model::column::Column;
use crate::model::column_type::ColumnType;
use crate::model::constraint::Constraint;
use crate::model::initial_data::InitialData;
use crate::model::key::Key;
use crate::model::relation::Relation;
use crate::model::trigger::Trigger;
use crate::model::types::{BooleanMode, KeyType, LockEscalation, TableOption};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Table {
    schema_name: Option<String>,
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
        schema_name: Option<S>,
        name: S,
        export_date_column: Option<S>,
        lock_escalation: LockEscalation,
        no_export: bool,
        columns: Vec<Column>,
        keys: Vec<Key>,
        indexes: Vec<Key>,
        relations: Vec<Relation>,
        triggers: Vec<Trigger>,
        constraints: Vec<Constraint>,
        initial_data: Vec<InitialData>,
        options: Vec<TableOption>,
        aggregations: Vec<Aggregation>,
    ) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            export_date_column: export_date_column.map(|s| s.into()),
            lock_escalation,
            no_export,
            columns,
            keys,
            indexes,
            relations,
            reverse_relations: Vec::new(),
            triggers,
            constraints,
            initial_data,
            options,
            aggregations,
        }
    }

    pub fn schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn export_date_column(&self) -> Option<&str> {
        self.export_date_column.as_deref()
    }

    pub fn lock_escalation(&self) -> LockEscalation {
        self.lock_escalation
    }

    pub fn is_no_export(&self) -> bool {
        self.no_export
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn indexes(&self) -> &[Key] {
        &self.indexes
    }

    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }

    pub fn reverse_relations(&self) -> &[Relation] {
        &self.reverse_relations
    }

    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    pub fn initial_data(&self) -> &[InitialData] {
        &self.initial_data
    }

    pub fn options(&self) -> &[TableOption] {
        &self.options
    }

    pub fn aggregations(&self) -> &[Aggregation] {
        &self.aggregations
    }

    pub fn column(&self, column_name: &str) -> &Column {
        let lower = column_name.to_lowercase();
        self.columns
            .iter()
            .find(|c| c.name().eq_ignore_ascii_case(&lower))
            .unwrap_or_else(|| {
                panic!(
                    "Unable to locate a column with the name '{}' in table '{}'",
                    column_name, self.name
                )
            })
    }

    pub fn primary_key(&self) -> Option<&Key> {
        self.keys.iter().find(|k| k.r#type() == KeyType::Primary)
    }

    pub fn has_column(&self, column_name: &str) -> bool {
        let lower = column_name.to_lowercase();
        self.columns
            .iter()
            .any(|c| c.name().eq_ignore_ascii_case(&lower))
    }

    pub fn identity_column(&self) -> Option<&Column> {
        self.columns.iter().find(|c| {
            let t = c.column_type();
            matches!(t, ColumnType::Sequence | ColumnType::LongSequence)
        })
    }

    pub fn primary_key_columns(&self) -> Option<Vec<String>> {
        self.primary_key()
            .map(|k| k.columns().iter().map(|kc| kc.name().to_string()).collect())
    }

    pub fn has_option(&self, option: TableOption) -> bool {
        self.options.iter().any(|o| *o == option)
    }

    pub fn has_column_constraints(&self, boolean_mode: BooleanMode) -> bool {
        self.columns
            .iter()
            .any(|c| c.needs_check_constraints(boolean_mode))
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

    pub fn fully_qualified_table_name(&self) -> String {
        match self.schema_name() {
            Some(schema_name) => format!("{}.{}", schema_name, self.name()),
            None => self.name().to_string(),
        }
    }

    pub fn add_reverse_relation(&mut self, relation: Relation) {
        self.reverse_relations.push(relation);
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.schema_name {
            Some(schema) => write!(f, "{}.{}", schema, self.name),
            None => write!(f, "{}", self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::key::{Key, KeyColumn};
    use crate::model::types::{BooleanMode, KeyType, LockEscalation, TableOption};

    fn sample_table() -> Table {
        Table::new(
            Some("schema"),
            "table",
            Option::<&str>::None,
            LockEscalation::Auto,
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
        )
    }

    #[test]
    fn constructor_and_basic_getters() {
        let t = sample_table();
        assert_eq!(t.schema_name().unwrap(), "schema");
        assert_eq!(t.name(), "table");
        assert_eq!(t.export_date_column(), None);
        assert_eq!(t.lock_escalation(), LockEscalation::Auto);
        assert!(!t.is_no_export());
        assert_eq!(format!("{}", t), "schema.table");
    }
}
