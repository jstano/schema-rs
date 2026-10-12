//! Dialect-neutral rows produced by the database readers and consumed by `reader.rs`.

use schema_model::model::column_type::ColumnType;

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub table_name: String,
    pub column_name: String,
    pub column_type: ColumnType,
    pub length: i32,
    pub scale: i32,
    pub required: bool,
    pub default_constraint: Option<String>,
    pub generated: Option<String>,
    pub enum_type: Option<String>,
    pub element_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct KeyInfo {
    pub columns: Vec<String>,
    /// Non-key `INCLUDE` columns (covering index).
    pub include: Vec<String>,
    /// Bare predicate of a partial index (`WHERE ...`).
    pub filter: Option<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TableKeys {
    pub primary: Option<KeyInfo>,
    pub unique: Vec<KeyInfo>,
    pub index: Vec<KeyInfo>,
}
