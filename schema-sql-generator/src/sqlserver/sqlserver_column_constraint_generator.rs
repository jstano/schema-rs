use schema_model::model::table::Table;
use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqlServerColumnConstraintGenerator {
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl SqlServerColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context),
        }
    }
}

impl ColumnConstraintGenerator for SqlServerColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        self.column_constraint_generator.column_check_constraints(table)
    }
}
