use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::{DefaultKeyGenerator, KeyGenerator};

pub struct SqliteKeyGenerator {
    key_generator: DefaultKeyGenerator,
}

impl SqliteKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            key_generator: DefaultKeyGenerator::new(context.clone()),
        }
    }
}

impl KeyGenerator for SqliteKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        self.key_generator.key_constraints(table)
    }
}
