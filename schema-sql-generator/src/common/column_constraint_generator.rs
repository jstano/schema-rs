use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;

pub trait ColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultColumnConstraintGenerator {
    context: GeneratorContext,
}

impl DefaultColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl ColumnConstraintGenerator for DefaultColumnConstraintGenerator {
    fn column_check_constraints(&self, _table: &Table) -> Vec<String> {
        vec![]
    }
}
