use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};

pub struct MySqlProcedureGenerator {
    context: GeneratorContext,
    procedure_generator: DefaultProcedureGenerator,
}

impl MySqlProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context.clone()),
            context,
        }
    }
}

impl ProcedureGenerator for MySqlProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }
}
