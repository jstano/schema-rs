use crate::model::schema::Schema;
use crate::model::types::{BooleanMode, ForeignKeyMode, Version};

#[derive(Debug, Default)]
pub struct DatabaseModel {
    version: Option<Version>,
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    schemas: Vec<Schema>,
}

impl DatabaseModel {
    pub fn new(version: Option<Version>, schemas: Vec<Schema>) -> Self {
        Self {
            version,
            boolean_mode: BooleanMode::Native,
            foreign_key_mode: ForeignKeyMode::Relations,
            schemas,
        }
    }

    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    pub fn foreign_key_mode(&self) -> ForeignKeyMode {
        self.foreign_key_mode
    }

    pub fn boolean_mode(&self) -> BooleanMode {
        self.boolean_mode
    }

    pub fn schemas(&self) -> &Vec<Schema> {
        &self.schemas
    }
}
