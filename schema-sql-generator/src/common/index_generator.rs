use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;

pub trait IndexGenerator {
    fn output_indexes(&self);
    fn output_indexes_for_table(&self, table: &Table);
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

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl IndexGenerator for DefaultIndexGenerator {
    fn output_indexes(&self) {
    }

    fn output_indexes_for_table(&self, _table: &Table) {
    }
}
