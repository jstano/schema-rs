use crate::common::generator_context::GeneratorContext;
use schema_model::model::constraint::Constraint;
use schema_model::model::table::Table;

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

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn generator_constraint(&self, constraint: &Constraint) -> String {
        format!("   constraint {} {}",
               constraint.name(),
               constraint.sql())
    }
}

impl TableConstraintGenerator for DefaultTableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String> {
        table.constraints().iter().map(|constraint| {
            self.generator_constraint(constraint)
        }).collect()
    }
}
