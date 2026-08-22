use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::ProcedureGenerator;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::procedure::Procedure;

pub struct SqliteProcedureGenerator {
    context: GeneratorContext,
}

impl SqliteProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }
}

impl ProcedureGenerator for SqliteProcedureGenerator {
    fn output_procedures(&self) {
        let database_type = self.context.settings().database_type();
        let database_model = self.context.settings().database_model();

        let has_procedures = database_model
            .schemas()
            .iter()
            .any(|schema| schema.procedures().iter().any(|p| p.database_type() == database_type));

        if has_procedures {
            panic!("SQLite does not support stored procedures.");
        }
    }

    fn output_procedure(&self, _writer: &mut SqlWriter, _statement_separator: &str, _procedure: &Procedure) {
        panic!("SQLite does not support stored procedures.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::procedure::Procedure;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_procedures_does_nothing_when_none_exist() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteProcedureGenerator::new(ctx);
        generator.output_procedures();

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    #[should_panic(expected = "SQLite does not support stored procedures.")]
    fn output_procedures_panics_when_a_sqlite_procedure_exists() {
        let procedure = Procedure::new(None::<&str>, "proc", DatabaseType::Sqlite, "select 1");
        let schema = SchemaBuilder::new(None::<&str>)
            .add_procedures(vec![procedure])
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteProcedureGenerator::new(ctx);
        generator.output_procedures();
    }
}
