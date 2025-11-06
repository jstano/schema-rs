use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::{DefaultKeyGenerator, KeyGenerator};

pub struct MySqlKeyGenerator {
    key_generator: DefaultKeyGenerator,
}

impl MySqlKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            key_generator: DefaultKeyGenerator::new(context.clone()),
        }
    }
}

impl KeyGenerator for MySqlKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        self.key_generator.key_constraints(table)
    }
}
