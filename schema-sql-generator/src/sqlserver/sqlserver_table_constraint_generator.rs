use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::table_constraint_generator::TableConstraintGenerator;

pub struct SqlServerTableConstraintGenerator {
    context: GeneratorContext,
}

impl SqlServerTableConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }   
}

impl TableConstraintGenerator for SqlServerTableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String> {
        todo!()
    }
}