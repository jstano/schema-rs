use crate::common::column_generator::{ColumnGenerator, DefaultColumnGenerator};
use crate::common::generator_context::GeneratorContext;
use crate::postgresql::postgres_column_type_generator::PostgresColumnTypeGenerator;
use schema_model::model::column::Column;
use schema_model::model::table::Table;

pub struct PostgresColumnGenerator {
    column_generator: DefaultColumnGenerator,
}

impl PostgresColumnGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_generator: DefaultColumnGenerator::new(
                context.clone(),
                Box::new(PostgresColumnTypeGenerator::new(context.clone())),
            ),
        }
    }
}

impl ColumnGenerator for PostgresColumnGenerator {
    fn column_definitions(&self, table: &Table) -> Vec<String> {
        self.column_generator.column_definitions(table)
    }

    fn column_sql(&self, table: &Table, column: &Column) -> String {
        self.column_generator.column_sql(table, column)
    }

    fn column_options(&self, table: &Table, column: &Column) -> String {
        self.column_generator.column_options(table, column)
    }

    fn default_value(&self, table: &Table, column: &Column) -> Option<String> {
        self.column_generator.default_value(table, column)
    }
}
