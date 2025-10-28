use schema_model::model::table::Table;
use crate::common::column_constraint_generator::ColumnConstraintGenerator;
use crate::common::generator_context::GeneratorContext;

pub struct H2ColumnConstraintGenerator {
    context: GeneratorContext,
}

impl H2ColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ColumnConstraintGenerator for H2ColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}
