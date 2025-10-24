use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;

pub trait IndexGenerator {
    fn output_indexes(&self);
    fn output_indexes_for_table(&mut self, table: &Table);
}

pub struct DefaultIndexGenerator {
    context: GeneratorContext,
}

impl DefaultIndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl IndexGenerator for DefaultIndexGenerator {
    fn output_indexes(&self) {
    }

    fn output_indexes_for_table(&mut self, table: &Table) {
    }
}
