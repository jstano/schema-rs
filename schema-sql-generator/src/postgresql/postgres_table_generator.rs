use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::postgresql::postgres_column_constraint_generator::PostgresColumnConstraintGenerator;
use crate::postgresql::postgres_column_generator::PostgresColumnGenerator;
use crate::postgresql::postgres_index_generator::PostgresIndexGenerator;
use crate::postgresql::postgres_key_generator::PostgresKeyGenerator;
use crate::postgresql::postgres_table_constraint_generator::PostgresTableConstraintGenerator;
use schema_model::model::table::Table;

pub struct PostgresTableGenerator {
    context: GeneratorContext,
    table_generator: DefaultTableGenerator,
}

impl PostgresTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(
                context.clone(),
                Box::new(PostgresColumnGenerator::new(context.clone())),
                Box::new(PostgresKeyGenerator::new(context.clone())),
                Box::new(PostgresColumnConstraintGenerator::new(context.clone())),
                Box::new(PostgresTableConstraintGenerator::new(context.clone())),
                Box::new(PostgresIndexGenerator::new(context.clone())),
            ),
            context,
        }
    }
}

impl TableGenerator for PostgresTableGenerator {
    fn output_tables(&self) {
        // Self-dispatching loop, not a delegation to `DefaultTableGenerator::output_tables` -
        // see `SqliteTableGenerator`/`SqlServerTableGenerator` for why: delegating would call
        // `self.output_table` on the `DefaultTableGenerator`, statically bypassing any future
        // override of `output_table`/its steps on this type.
        let database_model = self.context.settings().database_model();
        for schema in database_model.schemas() {
            for table in schema.tables() {
                self.output_table(table);
            }
        }
    }

    fn output_table(&self, table: &Table) {
        self.output_table_drop(table);
        self.output_table_header(table);
        self.output_table_definition(table);
        self.output_table_footer(table);
        self.output_indexes(table);
        self.output_initial_data(table);
    }

    fn output_table_drop(&self, table: &Table) {
        self.table_generator.output_table_drop(table);
    }

    fn output_table_header(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_definition(&self, table: &Table) {
        self.table_generator.output_table_definition(table);
    }

    fn output_table_footer(&self, table: &Table) {
        self.table_generator.output_table_footer(table);
    }

    fn output_indexes(&self, table: &Table) {
        self.table_generator.output_indexes(table);
    }

    fn output_initial_data(&self, table: &Table) {
        self.table_generator.output_initial_data(table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_table_renders_header_and_columns() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "name", ColumnType::Varchar).length(50).required(true).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresTableGenerator::new(ctx);
        generator.output_table_header(&table);
        generator.output_table_definition(&table);
        generator.output_table_footer(&table);

        let output = buffer.contents();
        assert!(output.contains("create table public.users"));
        assert!(output.contains("id serial"));
        assert!(output.contains("name text"));
        assert!(output.contains(");"));
    }
}
