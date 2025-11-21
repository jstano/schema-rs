use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::ProcedureGenerator;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::procedure::Procedure;

pub struct SqliteProcedureGenerator {
}

impl SqliteProcedureGenerator {
    pub fn new(_context: GeneratorContext) -> Self {
        Self {
        }
    }
}

impl ProcedureGenerator for SqliteProcedureGenerator {
    fn output_procedures(&self) {
        panic!("SQLite does not support stored procedures.");
    }

    fn output_procedure(&self, _writer: &mut SqlWriter, _statement_separator: &str, _procedure: &Procedure) {
        panic!("SQLite does not support stored procedures.");
    }
}
