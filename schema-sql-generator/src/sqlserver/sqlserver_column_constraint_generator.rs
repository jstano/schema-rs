use schema_model::model::table::Table;
use crate::common::column_constraint_generator::ColumnConstraintGenerator;
use crate::common::generator_context::GeneratorContext;

pub struct SqlServerColumnConstraintGenerator {
    context: GeneratorContext,
}

impl SqlServerColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ColumnConstraintGenerator for SqlServerColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}
