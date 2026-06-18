use crate::common::output_mode::OutputMode;
use crate::common::print_writer::PrintWriter;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use std::cell::RefCell;
use std::rc::Rc;

pub struct GenerateOptions {
    pub database_model: Rc<DatabaseModel>,
    pub writer: Rc<RefCell<PrintWriter>>,
    pub foreign_key_mode: ForeignKeyMode,
    pub boolean_mode: BooleanMode,
    pub output_mode: OutputMode,
    pub target_postgres_version: u32,
}

impl GenerateOptions {
    pub fn new(database_model: Rc<DatabaseModel>, writer: Rc<RefCell<PrintWriter>>) -> Self {
        Self {
            database_model,
            writer,
            foreign_key_mode: ForeignKeyMode::Relations,
            boolean_mode: BooleanMode::Native,
            output_mode: OutputMode::All,
            target_postgres_version: 0,
        }
    }
}
