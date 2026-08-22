use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};
use crate::common::sql_string::escape_sql_literal;
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
        // Route through `output_procedures_via(self)` rather than
        // `self.procedure_generator.output_procedures()`: the latter dispatches each
        // procedure statically on the inner `DefaultProcedureGenerator`, which would
        // never see this struct's `output_procedure` override (the drop-if-exists guard
        // below) - the same static-dispatch pitfall fixed for functions.
        self.procedure_generator.output_procedures_via(self);
    }

    fn output_procedure(&self, writer: &mut SqlWriter, statement_separator: &str, procedure: &Procedure) {
        let procedure_name = procedure.name();
        let schema_name = match procedure.schema_name() {
            Some(s) if s.eq_ignore_ascii_case("public") => "dbo",
            Some(s) => s,
            None => "dbo",
        };
        let fully_qualified_name = format!("{}.{}", schema_name, procedure_name);

        writer.println(format!(
            "if exists (select * from dbo.sysobjects where id = object_id(N'[{}].[{}]') and objectproperty(id, N'IsProcedure') = 1)",
            escape_sql_literal(schema_name),
            escape_sql_literal(procedure_name)
        ).as_str());
        writer.print(format!("drop procedure {}", fully_qualified_name).as_str());
        writer.println(statement_separator);
        writer.print(procedure.sql());
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
    fn output_procedures_renders_matching_database_type_only() {
        let schema = SchemaBuilder::new(None::<&str>)
            .add_procedures(vec![
                Procedure::new(None::<&str>, "mssql_only", DatabaseType::SqlServer, "create procedure mssql_only as begin end"),
                Procedure::new(None::<&str>, "pg_only", DatabaseType::Postgresql, "create procedure pg_only"),
            ])
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerProcedureGenerator::new(ctx);
        generator.output_procedures();

        let output = buffer.contents();
        assert!(output.contains("create procedure mssql_only as begin end"));
        assert!(!output.contains("pg_only"));
    }

    #[test]
    fn output_procedures_applies_the_drop_if_exists_guard_through_the_real_pipeline() {
        // Regression test: output_procedures() (the real pipeline entry point) must
        // route through the drop-if-exists override, not silently fall back to
        // DefaultProcedureGenerator's plain rendering via static dispatch. Re-running
        // generated SQL against a database that already has the procedure must not fail
        // with "there is already an object named ...".
        let schema = SchemaBuilder::new(None::<&str>)
            .add_procedures(vec![Procedure::new(
                None::<&str>,
                "mssql_proc",
                DatabaseType::SqlServer,
                "create procedure dbo.mssql_proc as begin return 1 end",
            )])
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerProcedureGenerator::new(ctx);
        generator.output_procedures();

        let output = buffer.contents();
        assert!(output.contains("if exists (select * from dbo.sysobjects where id = object_id(N'[dbo].[mssql_proc]') and objectproperty(id, N'IsProcedure') = 1)"));
        assert!(output.contains("drop procedure dbo.mssql_proc"));
        assert!(output.contains("create procedure dbo.mssql_proc as begin return 1 end"));
    }

    #[test]
    fn output_procedure_escapes_single_quote_in_procedure_name() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);
        let procedure = Procedure::new(
            None::<&str>,
            "o'brien",
            DatabaseType::SqlServer,
            "create procedure dbo.[o'brien] as begin return 1 end",
        );

        let generator = SqlServerProcedureGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_procedure(writer, ";", &procedure);
        });

        let output = buffer.contents();
        assert!(output.contains("object_id(N'[dbo].[o''brien]')"));
    }
}
