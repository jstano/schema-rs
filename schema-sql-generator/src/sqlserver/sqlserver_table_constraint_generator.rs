use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::table_constraint_generator::{DefaultTableConstraintGenerator, TableConstraintGenerator};

pub struct SqlServerTableConstraintGenerator {
    table_constraint_generator: DefaultTableConstraintGenerator,
}

impl SqlServerTableConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_constraint_generator: DefaultTableConstraintGenerator::new(context),
        }
    }   
}

impl TableConstraintGenerator for SqlServerTableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String> {
        self.table_constraint_generator.table_check_constraints(table)
    }
}
