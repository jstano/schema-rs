use crate::common::generate_options::GenerateOptions;
use crate::common::print_writer::PrintWriter;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::DatabaseType;
use std::cell::RefCell;
use std::rc::Rc;

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
