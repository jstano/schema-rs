use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqlServerFunctionGenerator {
    context: GeneratorContext,
    function_generator: DefaultFunctionGenerator
}

impl SqlServerFunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            function_generator: DefaultFunctionGenerator::new(context.clone()),
            context,
        }
    }
}

impl FunctionGenerator for SqlServerFunctionGenerator {
    fn output_functions(&self) {
        self.function_generator.output_functions();
    }
}
