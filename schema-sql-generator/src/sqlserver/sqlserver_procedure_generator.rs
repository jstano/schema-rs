use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};
use crate::common::sql_writer::SqlWriter;
use schema_model::model::procedure::Procedure;

pub struct SqlServerProcedureGenerator {
    procedure_generator: DefaultProcedureGenerator,
}

impl SqlServerProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context),
        }
    }
}

impl ProcedureGenerator for SqlServerProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }

    fn output_procedure(&self, writer: &mut SqlWriter, statement_separator: &str, procedure: &Procedure) {
        self.procedure_generator.output_procedure(writer, statement_separator, procedure);
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
    fn output_procedures_renders_matching_database_type_only() {
        let schema = SchemaBuilder::new(None::<&str>)
            .add_procedures(vec![
                Procedure::new(None::<&str>, "mssql_only", DatabaseType::SqlServer, "create procedure mssql_only as begin end"),
                Procedure::new(None::<&str>, "pg_only", DatabaseType::Postgresql, "create procedure pg_only"),
            ])
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerProcedureGenerator::new(ctx);
        generator.output_procedures();

        let output = buffer.contents();
        assert!(output.contains("create procedure mssql_only as begin end"));
        assert!(!output.contains("pg_only"));
    }
}
