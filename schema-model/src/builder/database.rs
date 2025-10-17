use crate::builder::{ColumnBuilder, KeyBuilder, TableBuilder};
use crate::model::column::Column;
use crate::model::column_type::ColumnType;
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::types::{BooleanMode, ForeignKeyMode, KeyType, LockEscalation, Version};

/// DatabaseBuilder is the root builder that accumulates database-level settings
/// and delegates all object creation to a single, selected Schema.
///
/// Per requirement, `build()` returns a fully-populated Schema (not a Database).
#[derive(Debug)]
pub struct DatabaseBuilder {
    // Database-level attributes (kept for completeness; not applied to Schema today)
    version: Option<Version>,
    foreign_key_mode: Option<ForeignKeyMode>,
    boolean_mode: Option<BooleanMode>,

    // Target schema we are building (name + inner builder)
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

    /// Set an optional database version.
    pub fn version(mut self, v: Version) -> Self { self.version = Some(v); self }
    /// Set database foreign key mode (stored on builder; schema currently doesn’t carry this).
    pub fn foreign_key_mode(mut self, m: ForeignKeyMode) -> Self { self.foreign_key_mode = Some(m); self }
    /// Set database boolean mode (stored on builder; schema currently doesn’t carry this).
    pub fn boolean_mode(mut self, m: BooleanMode) -> Self { self.boolean_mode = Some(m); self }

    /// Add a fully built table into the target schema.
    pub fn add_table(mut self, table: Table) -> Self { self.schema_builder = self.schema_builder.add_table(table); self }

    /// Convenience to add a table via a TableBuilder.
    pub fn add_table_built(mut self, tb: TableBuilder) -> Self { self.schema_builder = self.schema_builder.add_table(tb.build()); self }

    /// Build and return the fully populated Schema.
    pub fn build(self) -> Schema {
        // At the moment Schema does not model version/boolean/foreign key settings.
        // If those fields are added to Schema later, we can easily apply them here
        // before returning the built Schema.
        self.schema_builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_schema_through_database_builder() {
        // Build a simple table through the DB builder.
        let table = TableBuilder::new("public", "users")
            .add_column(ColumnBuilder::new("id", ColumnType::Int).required(true).build())
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
