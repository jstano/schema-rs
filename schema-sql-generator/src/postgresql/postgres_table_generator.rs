use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use schema_model::model::table::Table;

pub struct PostgresTableGenerator {
    context: GeneratorContext,
    table_generator: DefaultTableGenerator,
}

impl PostgresTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(context.clone()),
            context,
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
