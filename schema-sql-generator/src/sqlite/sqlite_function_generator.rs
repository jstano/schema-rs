use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqliteFunctionGenerator {
    context: GeneratorContext,
    function_generator: DefaultFunctionGenerator
}

impl SqliteFunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            function_generator: DefaultFunctionGenerator::new(context.clone()),
            context,
        }
    }
}

impl FunctionGenerator for SqliteFunctionGenerator {
    fn output_functions(&self) {
        self.function_generator.output_functions();
    }
}
