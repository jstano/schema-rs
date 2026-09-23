use crate::common::generate_options::GenerateOptions;
use crate::common::print_writer::PrintWriter;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::DatabaseType;
use std::cell::RefCell;
use std::io::Write as IoWrite;
use std::rc::Rc;

/// A `Write` sink backed by a shared, readable in-memory buffer - for callers (e.g. the
/// migration generator) that need to invoke a generator method for its side-effecting writes
/// (via `GeneratorContext::with_writer`) and then read back what it wrote, without a temp
/// file. Mirrors the `SharedBuffer` test helper in `test_support.rs`, but is not
/// `#[cfg(test)]` since production code outside this crate needs it too.
#[derive(Clone, Default)]
pub struct BufferSink(Rc<RefCell<Vec<u8>>>);

impl BufferSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns and clears the buffer's contents, so a caller emitting several separate
    /// chunks of generator output (e.g. one per table) can drain each chunk right after
    /// producing it and keep the surrounding output in the right order.
    pub fn take(&self) -> String {
        let mut buf = self.0.borrow_mut();
        String::from_utf8(std::mem::take(&mut *buf)).unwrap_or_default()
    }
}

impl IoWrite for BufferSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

#[derive(Clone)]
pub struct GeneratorContext {
    settings: Rc<SqlGeneratorSettings>,
    writer: Rc<RefCell<SqlWriter>>,
}

impl GeneratorContext {
    pub fn new(settings: SqlGeneratorSettings, writer: SqlWriter) -> Self {
        Self {
            settings: Rc::new(settings),
            writer: Rc::new(RefCell::new(writer)),
        }
    }

    /// Builds a context from just a model and dialect, with an in-memory output sink -
    /// for callers (e.g. `schema-migration-generator`) that only need to call column-level
    /// generator methods (column type / check constraint rendering), not run a full
    /// `CREATE TABLE` generation. `GenerateOptions::new`'s default `boolean_mode` is
    /// `Native`, so it's overridden here from `database_model.boolean_mode()` - the model
    /// always carries the real configured mode.
    pub fn for_model(database_model: Rc<DatabaseModel>, database_type: DatabaseType) -> Self {
        let boolean_mode = database_model.boolean_mode();
        let writer = Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new()))));
        let mut options = GenerateOptions::new(database_model, writer);
        options.boolean_mode = boolean_mode;
        let settings = SqlGeneratorSettings::new(database_type, &options);
        let sql_writer = SqlWriter::new(options.writer.clone());
        Self::new(settings, sql_writer)
    }

    /// Like `for_model`, but with the in-memory sink exposed as a readable `BufferSink`
    /// instead of a write-only, inaccessible `Vec<u8>` - for callers that need to invoke a
    /// generator method (e.g. `TriggerGenerator::output_triggers_for_table`) for its
    /// side-effecting writes and then read back what it produced.
    pub fn for_model_with_buffer(database_model: Rc<DatabaseModel>, database_type: DatabaseType) -> (Self, BufferSink) {
        let boolean_mode = database_model.boolean_mode();
        let buffer = BufferSink::new();
        let writer = Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone()))));
        let mut options = GenerateOptions::new(database_model, writer);
        options.boolean_mode = boolean_mode;
        let settings = SqlGeneratorSettings::new(database_type, &options);
        let sql_writer = SqlWriter::new(options.writer.clone());
        (Self::new(settings, sql_writer), buffer)
    }

    pub fn settings(&self) -> &SqlGeneratorSettings {
        &self.settings
    }

    pub fn shared_settings(&self) -> Rc<SqlGeneratorSettings> {
        Rc::clone(&self.settings)
    }

    pub fn writer(&self) -> Rc<RefCell<SqlWriter>> {
        Rc::clone(&self.writer)
    }

    pub fn with_writer<F>(&self, f: F)
    where
        F: FnOnce(&mut SqlWriter),
    {
        let mut writer = self.writer.borrow_mut();
        f(&mut writer);
    }
}
