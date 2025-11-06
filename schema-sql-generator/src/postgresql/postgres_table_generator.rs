use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use schema_model::model::table::Table;
use crate::postgresql::postgres_column_constraint_generator::PostgresColumnConstraintGenerator;
use crate::postgresql::postgres_column_generator::PostgresColumnGenerator;
use crate::postgresql::postgres_index_generator::PostgresIndexGenerator;
use crate::postgresql::postgres_key_generator::PostgresKeyGenerator;
use crate::postgresql::postgres_table_constraint_generator::PostgresTableConstraintGenerator;

pub struct PostgresTableGenerator {
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
        }
    }
}

impl TableGenerator for PostgresTableGenerator {
    fn output_tables(&self) {
        self.table_generator.output_tables();
    }

    fn output_table(&self, table: &Table) {
        self.table_generator.output_table_header(table);
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
