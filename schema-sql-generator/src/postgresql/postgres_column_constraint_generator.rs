use schema_model::model::table::Table;
use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct PostgresColumnConstraintGenerator {
    context: GeneratorContext,
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl PostgresColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context.clone()),
            context,
        }
    }
}

impl ColumnConstraintGenerator for PostgresColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        self.column_constraint_generator.column_check_constraints(table)
    }
}
