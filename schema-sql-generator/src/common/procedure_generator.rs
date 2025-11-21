use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::procedure::Procedure;

pub trait ProcedureGenerator {
    fn output_procedures(&self);
    fn output_procedure(
        &self,
        writer: &mut SqlWriter,
        statement_separator: &str,
        procedure: &Procedure,
    );
}

pub struct DefaultProcedureGenerator {
    context: GeneratorContext,
}

impl DefaultProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl ProcedureGenerator for DefaultProcedureGenerator {
    fn output_procedures(&self) {
        let database_type = self.context.settings().database_type();
        let statement_separator = self.context.settings().statement_separator();
        let database_model = self.context.settings().database_model();

        self.context.with_writer(|writer| {
            database_model.schemas().iter().for_each(|schema| {
                schema
                    .procedures()
                    .iter()
                    .filter(|procedure| procedure.database_type() == database_type)
                    .for_each(|procedure| {
                        self.output_procedure(writer, statement_separator, procedure);
                    })
            });
        });
    }

    fn output_procedure(&self, writer: &mut SqlWriter, statement_separator: &str, procedure: &Procedure) {
        writer.print(procedure.sql());
        writer.println(statement_separator);
        writer.newline();
    }
}
