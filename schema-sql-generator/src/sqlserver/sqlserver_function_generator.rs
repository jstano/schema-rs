use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::function::Function;

pub struct SqlServerFunctionGenerator {
    function_generator: DefaultFunctionGenerator
}

impl SqlServerFunctionGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            function_generator: DefaultFunctionGenerator::new(context),
        }
    }
}

impl FunctionGenerator for SqlServerFunctionGenerator {
    fn output_functions(&self) {
        self.function_generator.output_functions();
    }

    fn output_function(&self, writer: &mut SqlWriter, statement_separator: &str, function: &Function) {
        let function_name = function.name();

        writer.println(format!("if exists (select * from dbo.sysobjects where id = object_id(N'[dbo].[{}]') and objectproperty(id, N'IsScalarFunction') = 1)", function_name).as_str());
        writer.print(format!("drop function dbo.{}", function_name).as_str());
        writer.println(statement_separator);
        writer.print(function.sql());
        writer.println(statement_separator);
        writer.newline();
    }
}
