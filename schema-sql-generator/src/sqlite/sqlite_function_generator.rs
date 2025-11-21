use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::function::Function;

pub struct SqliteFunctionGenerator {
    function_generator: DefaultFunctionGenerator
}

impl SqliteFunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            function_generator: DefaultFunctionGenerator::new(context),
        }
    }
}

impl FunctionGenerator for SqliteFunctionGenerator {
    fn output_functions(&self) {
        self.function_generator.output_functions();
    }

    fn output_function(&self, writer: &mut SqlWriter, statement_separator: &str, function: &Function) {
        self.function_generator.output_function(writer, statement_separator, function);
    }
}
