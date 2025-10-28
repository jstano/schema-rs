use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use schema_model::model::table::Table;
use crate::h2::h2_column_constraint_generator::H2ColumnConstraintGenerator;
use crate::h2::h2_column_generator::H2ColumnGenerator;
use crate::h2::h2_index_generator::H2IndexGenerator;
use crate::h2::h2_key_generator::H2KeyGenerator;
use crate::h2::h2_table_constraint_generator::H2TableConstraintGenerator;

pub struct H2TableGenerator {
    context: GeneratorContext,
    table_generator: DefaultTableGenerator,
}

impl H2TableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(
                context.clone(),
                Box::new(H2ColumnGenerator::new(context.clone())),
                Box::new(H2KeyGenerator::new(context.clone())),
                Box::new(H2ColumnConstraintGenerator::new(context.clone())),
                Box::new(H2TableConstraintGenerator::new(context.clone())),
                Box::new(H2IndexGenerator::new(context.clone())),
            ),
            context,
        }
    }
}

impl TableGenerator for H2TableGenerator {
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
