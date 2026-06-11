use std::path::PathBuf;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use schema_sql_generator::common::generator_type::GeneratorType;
use crate::error::SchemaInstallerError;

pub struct SchemaInstallerConfig {
    pub database_type: GeneratorType,
    pub connection_string: String,
    pub schema_file: PathBuf,
    pub boolean_mode: BooleanMode,
    pub foreign_key_mode: ForeignKeyMode,
}

pub struct SchemaInstallerConfigBuilder {
    database_type: Option<GeneratorType>,
    connection_string: Option<String>,
    schema_file: Option<PathBuf>,
    boolean_mode: BooleanMode,
    foreign_key_mode: ForeignKeyMode,
}

impl SchemaInstallerConfigBuilder {
    pub fn new() -> Self {
        Self {
            database_type: None,
            connection_string: None,
            schema_file: None,
            boolean_mode: BooleanMode::Native,
            foreign_key_mode: ForeignKeyMode::Relations,
        }
    }

    pub fn database_type(mut self, db_type: GeneratorType) -> Self {
        self.database_type = Some(db_type);
        self
    }

    pub fn connection_string(mut self, conn_str: String) -> Self {
        self.connection_string = Some(conn_str);
        self
    }

    pub fn schema_file(mut self, path: PathBuf) -> Self {
        self.schema_file = Some(path);
        self
    }

    pub fn boolean_mode(mut self, mode: BooleanMode) -> Self {
        self.boolean_mode = mode;
        self
    }

    pub fn foreign_key_mode(mut self, mode: ForeignKeyMode) -> Self {
        self.foreign_key_mode = mode;
        self
    }

    pub fn build(self) -> Result<SchemaInstallerConfig, SchemaInstallerError> {
        let database_type = self.database_type
            .ok_or_else(|| SchemaInstallerError::InvalidConfiguration("database_type required".to_string()))?;
        let connection_string = self.connection_string
            .ok_or_else(|| SchemaInstallerError::InvalidConfiguration("connection_string required".to_string()))?;
        let schema_file = self.schema_file
            .ok_or_else(|| SchemaInstallerError::InvalidConfiguration("schema_file required".to_string()))?;

        if !schema_file.exists() {
            return Err(SchemaInstallerError::SchemaFileNotFound(schema_file.display().to_string()));
        }

        Ok(SchemaInstallerConfig {
            database_type,
            connection_string,
            schema_file,
            boolean_mode: self.boolean_mode,
            foreign_key_mode: self.foreign_key_mode,
        })
    }
}

impl Default for SchemaInstallerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
