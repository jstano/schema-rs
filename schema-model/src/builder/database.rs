use crate::builder::{TableBuilder};
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::types::{BooleanMode, ForeignKeyMode, Version};

/// DatabaseBuilder is the root builder that accumulates database-level settings
/// and delegates all object creation to a single, selected Schema.
///
/// Per requirement, `build()` returns a fully-populated Schema (not a Database).
#[derive(Debug)]
pub struct DatabaseBuilder {
    version: Option<Version>,
    foreign_key_mode: Option<ForeignKeyMode>,
    boolean_mode: Option<BooleanMode>,
    schema_name: String,
    schema_builder: crate::builder::SchemaBuilder,
}

impl DatabaseBuilder {
    /// Create a new DatabaseBuilder targeting a schema with the given name.
    pub fn new<S: Into<String>>(schema_name: S) -> Self {
        let name = schema_name.into();
        Self {
            version: None,
            foreign_key_mode: None,
            boolean_mode: None,
            schema_builder: crate::builder::SchemaBuilder::new(name.clone()),
            schema_name: name,
        }
    }

    pub fn version(mut self, v: Version) -> Self {
        self.version = Some(v);
        self
    }

    pub fn foreign_key_mode(mut self, m: ForeignKeyMode) -> Self {
        self.foreign_key_mode = Some(m);
        self
    }

    pub fn boolean_mode(mut self, m: BooleanMode) -> Self {
        self.boolean_mode = Some(m);
        self
    }

    pub fn add_table(mut self, table: Table) -> Self {
        self.schema_builder = self.schema_builder.add_table(table);
        self
    }

    pub fn add_table_built(mut self, tb: TableBuilder) -> Self {
        self.schema_builder = self.schema_builder.add_table(tb.build());
        self
    }

    pub fn build(self) -> Schema {
        self.schema_builder.build()
    }

    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::{ColumnBuilder, KeyBuilder};
    use crate::model::column_type::ColumnType;
    use crate::model::types::KeyType;
    use super::*;

    #[test]
    fn build_schema_through_database_builder() {
        // Build a simple table through the DB builder.
        let table = TableBuilder::new("public", "users")
            .add_column(
                ColumnBuilder::new("id", ColumnType::Int)
                    .required(true)
                    .build(),
            )
            .add_column(ColumnBuilder::new("name", ColumnType::Varchar).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();

        let schema = DatabaseBuilder::new("public")
            .version(Version::with_patch(1, 0, 0))
            .boolean_mode(BooleanMode::Native)
            .foreign_key_mode(ForeignKeyMode::Relations)
            .add_table(table)
            .build();

        assert_eq!(schema.schema_name(), "public");
        assert_eq!(schema.tables().len(), 1);
        assert_eq!(schema.tables()[0].name(), "users");
    }
}
