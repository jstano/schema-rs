use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::{DefaultKeyGenerator, KeyGenerator};
use schema_model::model::table::Table;

pub struct SqlServerKeyGenerator {
    key_generator: DefaultKeyGenerator,
}

impl SqlServerKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            key_generator: DefaultKeyGenerator::new(context).with_nonclustered_primary_key(true),
        }
    }
}

impl KeyGenerator for SqlServerKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        self.key_generator.key_constraints(table)
    }
}
