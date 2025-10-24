use crate::model::column::Column;
use crate::model::key::Key;
use crate::model::relation::Relation;
use crate::model::table::Table;
use crate::model::types::{LockEscalation, TableOption};

/// TableBuilder holds intermediate, mutable state for a table and produces an immutable Table.
#[derive(Debug)]
pub struct TableBuilder {
    schema_name: String,
    name: String,
    export_date_column: Option<String>,
    lock_escalation: LockEscalation,
    no_export: bool,

    columns: Vec<Column>,
    keys: Vec<Key>,
    indexes: Vec<Key>,
    relations: Vec<Relation>,
    options: Vec<TableOption>,
}

impl TableBuilder {
    pub fn new(schema_name: &str, name: &str) -> Self {
        Self {
            schema_name: if schema_name.is_empty() {
                "public".to_string()
            } else {
                schema_name.to_string()
            },
            name: name.into(),
            export_date_column: None,
            lock_escalation: LockEscalation::Auto,
            no_export: false,
            columns: Vec::new(),
            keys: Vec::new(),
            indexes: Vec::new(),
            relations: Vec::new(),
            options: Vec::new(),
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
    pub fn add_option(mut self, o: TableOption) -> Self {
        self.options.push(o);
        self
    }

    pub fn build(self) -> Table {
        let mut t = Table::new(
            self.schema_name,
            self.name,
            self.export_date_column,
            self.lock_escalation,
            self.no_export,
        );
        t.columns_mut().extend(self.columns);
        t.keys_mut().extend(self.keys);
        t.indexes_mut().extend(self.indexes);
        t.relations_mut().extend(self.relations);
        t.options_mut().extend(self.options);
        t
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
        let t = TableBuilder::new("s", "t")
            .add_column(Column::new("id", ColumnType::Int, 0, 0, true))
            .add_column(Column::new("name", ColumnType::Varchar, 255, 0, false))
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build();
        assert_eq!(t.name(), "t");
        assert_eq!(t.columns().len(), 2);
        assert!(t.primary_key().is_some());
    }
}
