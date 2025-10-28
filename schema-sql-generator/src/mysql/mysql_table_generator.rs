use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::mysql::mysql_column_constraint_generator::MySqlColumnConstraintGenerator;
use crate::mysql::mysql_column_generator::MySqlColumnGenerator;
use crate::mysql::mysql_key_generator::MySqlKeyGenerator;
use crate::mysql::mysql_table_constraint_generator::MySqlTableConstraintGenerator;
use schema_model::model::table::Table;
use crate::mysql::mysql_index_generator::MySqlIndexGenerator;

pub struct MySqlTableGenerator {
    context: GeneratorContext,
    table_generator: DefaultTableGenerator,
}

impl MySqlTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(
                context.clone(),
                Box::new(MySqlColumnGenerator::new(context.clone())),
                Box::new(MySqlKeyGenerator::new(context.clone())),
                Box::new(MySqlColumnConstraintGenerator::new(context.clone())),
                Box::new(MySqlTableConstraintGenerator::new(context.clone())),
                Box::new(MySqlIndexGenerator::new(context.clone())),
            ),
            context,
        }
    }
}

impl TableGenerator for MySqlTableGenerator {
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
