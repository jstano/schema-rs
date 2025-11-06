use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct PostgresFunctionGenerator {
    function_generator: DefaultFunctionGenerator
}

impl PostgresFunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            function_generator: DefaultFunctionGenerator::new(context),
        }
    }
}

impl FunctionGenerator for PostgresFunctionGenerator {
    fn output_functions(&self) {
        self.function_generator.output_functions();
    }
}
