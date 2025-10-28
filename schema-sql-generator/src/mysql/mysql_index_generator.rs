use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator};

pub struct MySqlIndexGenerator {
    context: GeneratorContext,
    index_generator: DefaultIndexGenerator,
}

impl MySqlIndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            index_generator: DefaultIndexGenerator::new(context.clone()),
            context,
        }
    }
}

impl IndexGenerator for MySqlIndexGenerator {
    fn output_indexes(&self) {
        self.index_generator.output_indexes();
    }

    fn output_indexes_for_table(&self, table: &Table) {
        self.index_generator.output_indexes_for_table(table);
    }
}
