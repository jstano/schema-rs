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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_function_renders_drop_if_exists_guard() {
        // Exercises SqlServerFunctionGenerator::output_function directly, since the normal
        // pipeline entry point (output_functions) does NOT reach this override: see the
        // dispatch-bug test below.
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);
        let function = Function::new(
            None::<&str>,
            "mssql_fn",
            DatabaseType::SqlServer,
            "create function dbo.mssql_fn() returns int as begin return 1 end",
        );

        let generator = SqlServerFunctionGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_function(writer, ";", &function);
        });

        let output = buffer.contents();
        assert!(output.contains("if exists (select * from dbo.sysobjects where id = object_id(N'[dbo].[mssql_fn]') and objectproperty(id, N'IsScalarFunction') = 1)"));
        assert!(output.contains("drop function dbo.mssql_fn"));
        assert!(output.contains("create function dbo.mssql_fn() returns int as begin return 1 end"));
    }

    #[test]
    fn output_functions_does_not_apply_the_drop_if_exists_override_due_to_static_dispatch() {
        // BUG: DefaultFunctionGenerator::output_functions() calls `self.output_function(...)`
        // on itself (a concrete DefaultFunctionGenerator), not through the FunctionGenerator
        // trait object, so SqlServerFunctionGenerator's drop-if-exists override above is never
        // reached through the real output_functions() pipeline entry point. This test documents
        // the current (buggy) behavior so a future fix is a visible, intentional test change.
        let schema = SchemaBuilder::new(None::<&str>)
            .add_functions(vec![Function::new(
                None::<&str>,
                "mssql_fn",
                DatabaseType::SqlServer,
                "create function dbo.mssql_fn() returns int as begin return 1 end",
            )])
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerFunctionGenerator::new(ctx);
        generator.output_functions();

        let output = buffer.contents();
        assert!(output.contains("create function dbo.mssql_fn() returns int as begin return 1 end"));
        assert!(!output.contains("drop function"), "if this now fails, the dispatch bug was fixed - update this test");
    }
}
