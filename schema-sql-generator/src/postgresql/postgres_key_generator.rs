use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::{DefaultKeyGenerator, KeyGenerator};

pub struct PostgresKeyGenerator {
    context: GeneratorContext,
    key_generator: DefaultKeyGenerator,
}

impl PostgresKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            key_generator: DefaultKeyGenerator::new(context.clone()),
            context,
        }
    }
}

impl KeyGenerator for PostgresKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        self.key_generator.key_constraints(table)
    }
}
