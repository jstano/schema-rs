use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::KeyGenerator;

pub struct H2KeyGenerator {
    context: GeneratorContext,
}

impl H2KeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl KeyGenerator for H2KeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}
