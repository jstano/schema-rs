use crate::model::enum_type::EnumType;
use crate::model::function::Function;
use crate::model::other_sql::OtherSql;
use crate::model::procedure::Procedure;
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::view::View;

/// SchemaBuilder accumulates intermediate state and produces an immutable Schema on build.
#[derive(Debug, Default)]
pub struct SchemaBuilder {
    schema: Schema,
}

impl SchemaBuilder {
    /// Create a new SchemaBuilder for a given schema name.
    pub fn new<S: Into<String>>(schema_name: S) -> Self {
        Self { schema: Schema::new(schema_name.into()) }
    }

    /// Add a fully prepared Table value.
    pub fn add_table(mut self, table: Table) -> Self { self.schema.add_table(table); self }
    /// Add a View.
    pub fn add_view(mut self, view: View) -> Self { self.schema.add_view(view); self }
    /// Add an enum type.
    pub fn add_enum_type(mut self, enum_type: EnumType) -> Self { self.schema.add_enum_type(enum_type); self }
    /// Add functions.
    pub fn add_functions(mut self, functions: Vec<Function>) -> Self { self.schema.add_functions(functions); self }
    /// Add procedures.
    pub fn add_procedures(mut self, procedures: Vec<Procedure>) -> Self { self.schema.add_procedures(procedures); self }
    /// Add an OtherSql.
    pub fn add_other_sql(mut self, other_sql: OtherSql) -> Self { self.schema.add_other_sql(other_sql); self }

    /// Finalize and return the fully-populated Schema.
    pub fn build(self) -> Schema { self.schema }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::column_type::ColumnType;
    use crate::builder::{TableBuilder, ColumnBuilder, KeyBuilder};
    use crate::model::types::KeyType;

    #[test]
    fn build_schema_with_table_and_pk() {
        let table = TableBuilder::new("public", "users")
            .add_column(ColumnBuilder::new("id", ColumnType::Int).required(true).build())
            .add_column(ColumnBuilder::new("name", ColumnType::Varchar).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();

        let schema = SchemaBuilder::new("public")
            .add_table(table)
            .build();

        assert_eq!(schema.tables().len(), 1);
        assert_eq!(schema.tables()[0].name(), "users");
    }
}
