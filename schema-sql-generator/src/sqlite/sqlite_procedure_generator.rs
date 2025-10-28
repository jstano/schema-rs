use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};

pub struct SqliteProcedureGenerator {
    context: GeneratorContext,
    procedure_generator: DefaultProcedureGenerator,
}

impl SqliteProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context.clone()),
            context,
        }
    }
}

impl ProcedureGenerator for SqliteProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }
}
