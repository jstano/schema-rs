use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::function::Function;

pub trait FunctionGenerator {
    fn output_functions(&self);
    fn output_function(
        &self,
        writer: &mut SqlWriter,
        statement_separator: &str,
        function: &Function,
    );

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
        let database_type = self.context.settings().database_type();
        let statement_separator = self.context.settings().statement_separator();
        let database_model = self.context.settings().database_model();

        self.context.with_writer(|writer| {
            database_model.schemas().iter().for_each(|schema| {
                schema
                    .functions()
                    .iter()
                    .filter(|function| function.database_type() == database_type)
                    .for_each(|function| {
                        self.output_function(writer, statement_separator, function);
                    })
            });
        });
    }

    fn output_function(&self, writer: &mut SqlWriter, statement_separator: &str, function: &Function) {
        writer.print(function.sql());
        writer.println(statement_separator);
        writer.newline();
    }
}
