use crate::common::generator_context::GeneratorContext;

pub trait ProcedureGenerator {
    fn output_procedures(&self);
}

pub struct DefaultProcedureGenerator {
    context: GeneratorContext,
}

impl DefaultProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ProcedureGenerator for DefaultProcedureGenerator {
    fn output_procedures(&self) {
    }
}