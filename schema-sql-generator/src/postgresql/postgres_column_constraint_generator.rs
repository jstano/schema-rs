use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;

pub struct PostgresColumnConstraintGenerator {
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl PostgresColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context),
        }
    }
}

impl ColumnConstraintGenerator for PostgresColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        let boolean_mode = self.column_constraint_generator.context().settings().boolean_mode();
        table.columns_with_check_constraints(boolean_mode)
            .iter()
            .filter(|col| col.column_type() != ColumnType::Enum)
            .map(|col| self.column_constraint_generator.generate_constraint(table, col))
            .collect()
    }
}
