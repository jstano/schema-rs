use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::KeyGenerator;

pub struct SqliteKeyGenerator {
    context: GeneratorContext,
}

impl SqliteKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl KeyGenerator for SqliteKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}
