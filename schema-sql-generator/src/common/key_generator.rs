use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;

pub trait KeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultKeyGenerator {
    context: GeneratorContext,
}

impl DefaultKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl KeyGenerator for DefaultKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        vec![]
    }
}
