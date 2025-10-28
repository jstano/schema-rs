use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct H2FunctionGenerator {
    context: GeneratorContext,
    function_generator: DefaultFunctionGenerator
}

impl H2FunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            function_generator: DefaultFunctionGenerator::new(context.clone()),
            context,
        }
    }
}

impl FunctionGenerator for H2FunctionGenerator {
    fn output_functions(&self) {
        self.function_generator.output_functions();
    }
}
