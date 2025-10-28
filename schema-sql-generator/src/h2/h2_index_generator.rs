use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator};

pub struct H2IndexGenerator {
    context: GeneratorContext,
    index_generator: DefaultIndexGenerator,
}

impl H2IndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            index_generator: DefaultIndexGenerator::new(context.clone()),
            context,
        }
    }
}

impl IndexGenerator for H2IndexGenerator {
    fn output_indexes(&self) {
        self.index_generator.output_indexes();
    }

    fn output_indexes_for_table(&self, table: &Table) {
        self.index_generator.output_indexes_for_table(table);
    }
}
