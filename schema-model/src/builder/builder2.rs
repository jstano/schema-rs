use crate::model::column::Column;
use crate::model::column_type::ColumnType;
use crate::model::function::Function;
use crate::model::key::{Key, KeyColumn};
use crate::model::procedure::Procedure;
use crate::model::relation::Relation;
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::types::{BooleanMode, ForeignKeyMode, KeyType, LockEscalation, Version};
use crate::model::view::View;
use crate::model::other_sql::OtherSql;

#[derive(Debug, Default)]
pub struct SchemaBuilder {
    schema: Schema,
}

impl SchemaBuilder {
    pub fn new<S: Into<String>>(schema_name: S) -> Self {
        Self { schema: Schema::new(schema_name.into()) }
    }

    pub fn version(mut self, version: Version) -> Self { self.schema.set_version(version); self }
    pub fn foreign_key_mode(mut self, mode: ForeignKeyMode) -> Self { self.schema.set_foreign_key_mode(mode); self }
    pub fn boolean_mode(mut self, mode: BooleanMode) -> Self { self.schema.set_boolean_mode(mode); self }

    pub fn add_table(mut self, table: Table) -> Self { self.schema.add_table(table); self }
    pub fn add_view(mut self, view: View) -> Self { self.schema.add_view(view); self }
    pub fn add_enum_type(mut self, enum_type: crate::model::enum_type::EnumType) -> Self { self.schema.add_enum_type(enum_type); self }
    pub fn add_functions(mut self, functions: Vec<Function>) -> Self { self.schema.add_functions(functions); self }
    pub fn add_procedures(mut self, procedures: Vec<Procedure>) -> Self { self.schema.add_procedures(procedures); self }
    pub fn add_other_sql(mut self, other_sql: OtherSql) -> Self { self.schema.add_other_sql(other_sql); self }

    pub fn build(self) -> Schema { self.schema }
}

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
    options: Vec<crate::model::types::TableOption>,
}

impl TableBuilder {
    pub fn new<S: Into<String>>(schema_name: S, name: S) -> Self {
        Self {
            schema_name: schema_name.into(),
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

    pub fn export_date_column<S: Into<String>>(mut self, col: S) -> Self { self.export_date_column = Some(col.into()); self }
    pub fn lock_escalation(mut self, le: LockEscalation) -> Self { self.lock_escalation = le; self }
    pub fn no_export(mut self, v: bool) -> Self { self.no_export = v; self }

    pub fn add_column(mut self, column: Column) -> Self { self.columns.push(column); self }
    pub fn add_key(mut self, key: Key) -> Self { self.keys.push(key); self }
    pub fn add_index(mut self, index: Key) -> Self { self.indexes.push(index); self }
    pub fn add_relation(mut self, relation: Relation) -> Self { self.relations.push(relation); self }
    pub fn add_option(mut self, o: crate::model::types::TableOption) -> Self { self.options.push(o); self }

    pub fn build(self) -> Table {
        let mut t = Table::new(
            self.schema_name,
            self.name,
            self.export_date_column,
            self.lock_escalation,
            self.no_export,
        );
        // fill collections
        t.columns_mut().extend(self.columns);
        t.keys_mut().extend(self.keys);
        t.indexes_mut().extend(self.indexes);
        t.relations_mut().extend(self.relations);
        t.options_mut().extend(self.options);
        t
    }
}

#[derive(Debug)]
pub struct ColumnBuilder {
    name: String,
    column_type: ColumnType,
    length: i32,
    scale: i32,
    required: bool,
}

impl ColumnBuilder {
    pub fn new<N: Into<String>>(name: N, column_type: ColumnType) -> Self {
        Self { name: name.into(), column_type, length: 0, scale: 0, required: false }
    }
    pub fn length(mut self, len: i32) -> Self { self.length = len; self }
    pub fn scale(mut self, sc: i32) -> Self { self.scale = sc; self }
    pub fn required(mut self, r: bool) -> Self { self.required = r; self }

    pub fn build(self) -> Column {
        Column::new(self.name, self.column_type, self.length, self.scale, self.required)
    }
}

#[derive(Debug)]
pub struct KeyBuilder {
    key_type: KeyType,
    columns: Vec<KeyColumn>,
    cluster: bool,
    compress: bool,
    unique: bool,
    include: Option<String>,
}

impl KeyBuilder {
    pub fn new(key_type: KeyType) -> Self {
        Self { key_type, columns: Vec::new(), cluster: false, compress: false, unique: false, include: None }
    }
    pub fn add_column<S: Into<String>>(mut self, name: S) -> Self { self.columns.push(KeyColumn::new(name)); self }
    pub fn cluster(mut self, v: bool) -> Self { self.cluster = v; self }
    pub fn compress(mut self, v: bool) -> Self { self.compress = v; self }
    pub fn unique(mut self, v: bool) -> Self { self.unique = v; self }
    pub fn include<S: Into<String>>(mut self, s: S) -> Self { self.include = Some(s.into()); self }

    pub fn build(self) -> Key {
        if self.cluster || self.compress || self.unique || self.include.is_some() {
            Key::new_full(self.key_type, self.columns, self.cluster, self.compress, self.unique, self.include)
        } else {
            Key::new(self.key_type, self.columns)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_schema_with_table_and_pk() {
        let table = TableBuilder::new("public", "users")
            .add_column(ColumnBuilder::new("id", ColumnType::Int).required(true).build())
            .add_column(ColumnBuilder::new("name", ColumnType::Varchar).length(100).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();

        let schema = SchemaBuilder::new("public")
            .version(Version::with_patch(1, 2, 3))
            .boolean_mode(BooleanMode::Native)
            .foreign_key_mode(ForeignKeyMode::Relations)
            .add_table(table)
            .build();

        assert_eq!(schema.tables().len(), 1);
        let t = &schema.tables()[0];
        assert_eq!(t.name(), "users");
        assert_eq!(t.get_primary_key().unwrap().columns().len(), 1);
    }
}
