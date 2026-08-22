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

    /// Same iteration/filtering logic as `output_procedures`, but dispatches each
    /// procedure through `generator` rather than `self`. Rust has no virtual dispatch on
    /// concrete types, so a dialect wrapper (e.g. `SqlServerProcedureGenerator`) that
    /// overrides `output_procedure` but delegates `output_procedures` straight to this
    /// struct would otherwise never see its own override invoked - callers that need
    /// their override honored should pass `self` (as a `&dyn ProcedureGenerator`) here
    /// instead of calling `output_procedures` directly.
    pub fn output_procedures_via(&self, generator: &dyn ProcedureGenerator) {
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
                        generator.output_procedure(writer, statement_separator, procedure);
                    })
            });
        });
    }
}

impl ProcedureGenerator for DefaultProcedureGenerator {
    fn output_procedures(&self) {
        self.output_procedures_via(self);
    }

    fn output_procedure(&self, writer: &mut SqlWriter, statement_separator: &str, procedure: &Procedure) {
        writer.print(procedure.sql());
        writer.println(statement_separator);
        writer.newline();
    }
}
