#![cfg(test)]

use crate::common::generate_options::GenerateOptions;
use crate::common::generator_context::GeneratorContext;
use crate::common::print_writer::PrintWriter;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{DatabaseType, ForeignKeyMode};
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contents(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

pub fn make_context(model: DatabaseModel, database_type: DatabaseType) -> (GeneratorContext, SharedBuffer) {
    let buffer = SharedBuffer::new();
    let options = GenerateOptions::new(
        Rc::new(model),
        Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
    );
    let settings = SqlGeneratorSettings::new(database_type, &options);
    let writer = SqlWriter::new(options.writer.clone());
    (GeneratorContext::new(settings, writer), buffer)
}

pub fn make_context_with_fk_mode(
    model: DatabaseModel,
    database_type: DatabaseType,
    foreign_key_mode: ForeignKeyMode,
) -> (GeneratorContext, SharedBuffer) {
    let buffer = SharedBuffer::new();
    let mut options = GenerateOptions::new(
        Rc::new(model),
        Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
    );
    options.foreign_key_mode = foreign_key_mode;
    let settings = SqlGeneratorSettings::new(database_type, &options);
    let writer = SqlWriter::new(options.writer.clone());
    (GeneratorContext::new(settings, writer), buffer)
}
