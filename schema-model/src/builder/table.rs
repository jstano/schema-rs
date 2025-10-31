use crate::model::aggregation::Aggregation;
use crate::model::column::Column;
use crate::model::constraint::Constraint;
use crate::model::initial_data::InitialData;
use crate::model::key::Key;
use crate::model::relation::Relation;
use crate::model::table::Table;
use crate::model::trigger::Trigger;
use crate::model::types::{LockEscalation, TableOption};

/// TableBuilder holds an intermediate, mutable state for a table and produces an immutable Table.
#[derive(Debug)]
pub struct TableBuilder {
    schema_name: Option<String>,
    name: String,
    export_date_column: Option<String>,
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
}

impl TableBuilder {
    pub fn new<S: Into<String>>(schema_name: Option<S>, name: S) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            export_date_column: None,
            lock_escalation: LockEscalation::Auto,
            no_export: false,
            columns: Vec::new(),
            keys: Vec::new(),
            indexes: Vec::new(),
            relations: Vec::new(),
            triggers: Vec::new(),
            constraints: Vec::new(),
            initial_data: Vec::new(),
            options: Vec::new(),
            aggregations: Vec::new(),
        }
    }

    pub fn export_date_column<S: Into<String>>(mut self, col: S) -> Self {
        self.export_date_column = Some(col.into());
        self
    }

    pub fn lock_escalation(mut self, le: LockEscalation) -> Self {
        self.lock_escalation = le;
        self
    }

    pub fn no_export(mut self, v: bool) -> Self {
        self.no_export = v;
        self
    }

    pub fn add_column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    pub fn add_key(mut self, key: Key) -> Self {
        self.keys.push(key);
        self
    }

    pub fn add_index(mut self, index: Key) -> Self {
        self.indexes.push(index);
        self
    }

    pub fn add_relation(mut self, relation: Relation) -> Self {
        self.relations.push(relation);
        self
    }

    pub fn add_trigger(mut self, trigger: Trigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    pub fn add_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    pub fn add_initial_data(mut self, initial_data: InitialData) -> Self {
        self.initial_data.push(initial_data);
        self
    }

    pub fn add_option(mut self, o: TableOption) -> Self {
        self.options.push(o);
        self
    }

    pub fn add_aggregation(mut self, aggregation: Aggregation) -> Self {
        self.aggregations.push(aggregation);
        self
    }

    pub fn build(self) -> Table {
        Table::new(
            self.schema_name,
            self.name,
            self.export_date_column,
            self.lock_escalation,
            self.no_export,
            self.columns,
            self.keys,
            self.indexes,
            self.relations,
            self.triggers,
            self.constraints,
            self.initial_data,
            self.options,
            self.aggregations,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::column::Column;
    use crate::model::column_type::ColumnType;
    use crate::model::key::{Key, KeyColumn};
    use crate::model::types::KeyType;

    #[test]
    fn build_table_with_columns_and_pk() {
        let t = TableBuilder::new(Some("s"), "t")
            .add_column(Column::new(Some("s"), "id", ColumnType::Int, 0, 0, true))
            .add_column(Column::new(Some("s"), "name", ColumnType::Varchar, 255, 0, false))
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build();
        assert_eq!(t.name(), "t");
        assert_eq!(t.columns().len(), 2);
        assert!(t.primary_key().is_some());
    }
}
