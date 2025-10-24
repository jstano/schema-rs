use std::cell::RefCell;
use std::rc::Rc;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;

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
