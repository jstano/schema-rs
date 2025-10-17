use crate::model::schema::Schema;
use crate::model::types::{BooleanMode, ForeignKeyMode, Version};

#[derive(Debug, Default)]
pub struct Database {
    version: Option<Version>,
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    schemas: Vec<Schema>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            version: None,
            boolean_mode: BooleanMode::Native,
            foreign_key_mode: ForeignKeyMode::Relations,
            schemas: vec![],
        }
    }

    pub fn version(&self) -> Option<&Version> { self.version.as_ref() }

    pub fn foreign_key_mode(&self) -> ForeignKeyMode { self.foreign_key_mode }

    pub fn boolean_mode(&self) -> BooleanMode { self.boolean_mode }

    pub fn schemas(&self) -> &Vec<Schema> {
        &self.schemas
    }
}
