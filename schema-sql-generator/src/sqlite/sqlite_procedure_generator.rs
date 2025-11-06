use crate::common::generator_context::GeneratorContext;
use crate::common::procedure_generator::ProcedureGenerator;

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
}
