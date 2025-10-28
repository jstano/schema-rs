use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};

pub struct SqlServerProcedureGenerator {
    context: GeneratorContext,
    procedure_generator: DefaultProcedureGenerator,
}

impl SqlServerProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context.clone()),
            context,
        }
    }
}

impl ProcedureGenerator for SqlServerProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }
}
