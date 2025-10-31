use crate::model::enum_type::EnumType;
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::types::{BooleanMode, ForeignKeyMode, Version};

#[derive(Debug, Default)]
pub struct DatabaseModel {
    version: Option<Version>,
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    schemas: Vec<Schema>,
}

impl DatabaseModel {
    pub fn new(version: Option<Version>,
               boolean_mode: BooleanMode,
               foreign_key_mode: ForeignKeyMode,
               schemas: Vec<Schema>) -> Self {
        Self {
            version,
            boolean_mode,
            foreign_key_mode,
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

    pub fn default_schema(&self) -> &Schema {
        self.schemas
            .iter()
            .find(|s| s.schema_name().is_none())
            .expect("Default schema not found")
    }

    pub fn default_schema_mut(&mut self) -> &mut Schema {
        self.schemas
            .iter_mut()
            .find(|s| s.schema_name().is_none())
            .expect("Default schema not found")
    }

    pub fn find_schema(&self, schema_name: Option<&str>) -> &Schema {
        if schema_name.is_none() {
            return self.default_schema();
        }

        self.schemas
            .iter()
            .filter(|s| s.schema_name().is_some())
            .find(|s| s.schema_name().unwrap() == schema_name.unwrap())
            .expect("Schema not found")
    }

    pub(crate) fn find_schema_mut(&mut self, schema_name: Option<&str>) -> &mut Schema {
        if schema_name.is_none() {
            return self.default_schema_mut();
        }

        self.schemas
            .iter_mut()
            .filter(|s| s.schema_name().is_some())
            .find(|s| s.schema_name().unwrap() == schema_name.unwrap())
            .expect("Schema not found")
    }

    pub fn find_enum_type(&self, schema_name: Option<&str>, enum_type: &str) -> &EnumType {
        let schema = self.find_schema(schema_name);
        schema.get_enum_type(enum_type)
    }

    pub fn all_tables(&self) -> Vec<&Table> {
        self.schemas
            .iter()
            .flat_map(|s| s.tables())
            .collect()
    }

    pub fn find_table(&self, schema_name: Option<&str>, table_name: &str) -> &Table {
        let schema = self.find_schema(schema_name);
        schema.get_table(table_name)
    }

    pub fn find_table_mut(&mut self, schema_name: Option<&str>, table_name: &str) -> &mut Table {
        let schema = self.find_schema_mut(schema_name);
        schema.get_table_mut(table_name)
    }
}
