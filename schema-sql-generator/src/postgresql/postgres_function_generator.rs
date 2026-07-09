use crate::common::function_generator::{DefaultFunctionGenerator, FunctionGenerator};
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::function::Function;

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

    fn output_function(&self, writer: &mut SqlWriter, statement_separator: &str, function: &Function) {
        self.function_generator.output_function(writer, statement_separator, function);
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
    fn output_functions_renders_matching_database_type_only() {
        let schema = SchemaBuilder::new(None::<&str>)
            .add_functions(vec![
                Function::new(None::<&str>, "pg_only", DatabaseType::Postgresql, "create function pg_only() returns void as $$ begin end $$ language plpgsql"),
                Function::new(None::<&str>, "sqlite_only", DatabaseType::Sqlite, "create function sqlite_only"),
            ])
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresFunctionGenerator::new(ctx);
        generator.output_functions();

        let output = buffer.contents();
        assert!(output.contains("create function pg_only()"));
        assert!(!output.contains("sqlite_only"));
    }
}
