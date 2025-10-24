use std::rc::Rc;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
use crate::common::generate_options::GenerateOptions;
use crate::common::output_mode::OutputMode;

#[derive(Debug, Clone)]
pub struct SqlGeneratorSettings {
    database_type: DatabaseType,
    database_model: Rc<DatabaseModel>,
    statement_separator: String,
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    output_mode: OutputMode,
}

impl SqlGeneratorSettings {
    pub fn new(database_type: DatabaseType, options: &GenerateOptions) -> Self {
        Self {
            database_type,
            database_model : options.database_model.clone(),
            statement_separator: ";".to_string(),
            foreign_key_mode: options.foreign_key_mode,
            boolean_mode: options.boolean_mode,
            output_mode: options.output_mode,
        }
    }

    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }

    pub fn database_model(&self) -> &Rc<DatabaseModel> {
        &self.database_model
    }

    pub fn statement_separator(&self) -> &String {
        &self.statement_separator
    }

    pub fn foreign_key_mode(&self) -> ForeignKeyMode {
        self.foreign_key_mode
    }

    pub fn boolean_mode(&self) -> BooleanMode {
        self.boolean_mode
    }

    pub fn output_mode(&self) -> OutputMode {
        self.output_mode
    }
}
