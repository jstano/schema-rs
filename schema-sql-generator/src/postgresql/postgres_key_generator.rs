use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::KeyGenerator;

pub struct PostgresKeyGenerator {
    context: GeneratorContext,
}

impl PostgresKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl KeyGenerator for PostgresKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}
