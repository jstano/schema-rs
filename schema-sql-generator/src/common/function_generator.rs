use crate::common::generator_context::GeneratorContext;

pub trait FunctionGenerator {
    fn output_functions(&self);
}

pub struct DefaultFunctionGenerator {
    context: GeneratorContext,
}

impl DefaultFunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl FunctionGenerator for DefaultFunctionGenerator {
    fn output_functions(&self) {
    }
}
