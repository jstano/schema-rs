use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};

pub struct PostgresProcedureGenerator {
    context: GeneratorContext,
    procedure_generator: DefaultProcedureGenerator,
}

impl PostgresProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context.clone()),
            context,
        }
    }
}

impl ProcedureGenerator for PostgresProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }
}
