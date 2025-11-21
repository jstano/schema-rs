use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::{DefaultProcedureGenerator, ProcedureGenerator};
use crate::common::sql_writer::SqlWriter;
use schema_model::model::procedure::Procedure;

pub struct H2ProcedureGenerator {
    procedure_generator: DefaultProcedureGenerator,
}

impl H2ProcedureGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            procedure_generator: DefaultProcedureGenerator::new(context.clone()),
        }
    }
}

impl ProcedureGenerator for H2ProcedureGenerator {
    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }

    fn output_procedure(&self, writer: &mut SqlWriter, statement_separator: &str, procedure: &Procedure) {
        self.procedure_generator.output_procedure(writer, statement_separator, procedure);
    }
}
