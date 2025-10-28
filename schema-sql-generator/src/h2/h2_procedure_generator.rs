use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};

pub struct H2ProcedureGenerator {
    context: GeneratorContext,
    procedure_generator: DefaultProcedureGenerator,
}

impl H2ProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context.clone()),
            context,
        }
    }
}

impl ProcedureGenerator for H2ProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }
}
