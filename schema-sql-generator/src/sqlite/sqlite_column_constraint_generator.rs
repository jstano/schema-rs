use schema_model::model::table::Table;
use crate::common::column_constraint_generator::ColumnConstraintGenerator;
use crate::common::generator_context::GeneratorContext;

pub struct SqliteColumnConstraintGenerator {
    context: GeneratorContext,
}

impl SqliteColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ColumnConstraintGenerator for SqliteColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}
