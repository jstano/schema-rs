use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::{DefaultKeyGenerator, KeyGenerator};

pub struct H2KeyGenerator {
    key_generator: DefaultKeyGenerator,
}

impl H2KeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            key_generator: DefaultKeyGenerator::new(context),
        }
    }
}

impl KeyGenerator for H2KeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        self.key_generator.key_constraints(table)
    }
}
