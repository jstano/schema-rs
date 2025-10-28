use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;

pub trait TableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultTableConstraintGenerator {
    context: GeneratorContext,
}

impl DefaultTableConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl TableConstraintGenerator for DefaultTableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String> {
        vec![]
    }
}
