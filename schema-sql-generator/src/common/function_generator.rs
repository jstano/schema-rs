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

    /// Same iteration/filtering logic as `output_functions`, but dispatches each
    /// function through `generator` rather than `self`. Rust has no virtual dispatch on
    /// concrete types, so a dialect wrapper (e.g. `SqlServerFunctionGenerator`) that
    /// overrides `output_function` but delegates `output_functions` straight to this
    /// struct would otherwise never see its own override invoked - callers that need
    /// their override honored should pass `self` (as a `&dyn FunctionGenerator`) here
    /// instead of calling `output_functions` directly.
    pub fn output_functions_via(&self, generator: &dyn FunctionGenerator) {
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
                        generator.output_function(writer, statement_separator, function);
                    })
            });
        });
    }
}

impl FunctionGenerator for DefaultFunctionGenerator {
    fn output_functions(&self) {
        self.output_functions_via(self);
    }

    fn output_function(&self, writer: &mut SqlWriter, statement_separator: &str, function: &Function) {
        writer.print(function.sql());
        writer.println(statement_separator);
        writer.newline();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    struct TaggingFunctionGenerator(DefaultFunctionGenerator);

    impl FunctionGenerator for TaggingFunctionGenerator {
        fn output_functions(&self) {
            self.0.output_functions_via(self);
        }

        fn output_function(&self, writer: &mut SqlWriter, statement_separator: &str, function: &Function) {
            writer.print("-- tagged\n");
            self.0.output_function(writer, statement_separator, function);
        }
    }

    #[test]
    fn output_functions_via_dispatches_through_the_passed_generator_not_self() {
        // Regression test for M10: a wrapper around `DefaultFunctionGenerator` must have
        // its own `output_function` override honored when `output_functions` calls
        // `output_functions_via(self)` rather than plainly delegating to
        // `DefaultFunctionGenerator::output_functions` - the latter would call back only
        // into `DefaultFunctionGenerator::output_function`, silently skipping the
        // wrapper's override, exactly the trap that made `SqlServerFunctionGenerator`'s
        // drop-guard unreachable before it was fixed.
        let schema = SchemaBuilder::new(None::<&str>)
            .add_functions(vec![Function::new(None::<&str>, "f", DatabaseType::Postgresql, "create function f()")])
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = TaggingFunctionGenerator(DefaultFunctionGenerator::new(ctx));
        generator.output_functions();

        let output = buffer.contents();
        assert!(output.contains("-- tagged"));
        assert!(output.contains("create function f()"));
    }
}
