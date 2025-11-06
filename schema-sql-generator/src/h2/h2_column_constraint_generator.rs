use schema_model::model::table::Table;
use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct H2ColumnConstraintGenerator {
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl H2ColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context),
        }
    }
}

impl ColumnConstraintGenerator for H2ColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        self.column_constraint_generator.column_check_constraints(table)
    }
}
