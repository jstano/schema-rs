use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename = "database")]
pub struct DatabaseXml {
    #[serde(rename = "@version")]
    pub version: Option<String>,

    #[serde(rename = "@foreignKeyMode")]
    pub foreign_key_mode: Option<String>,

    #[serde(rename = "@booleanMode")]
    pub boolean_mode: Option<String>,

    // Choice children
    #[serde(default)]
    #[serde(rename = "table")] pub tables: Vec<TableXml>,
    #[serde(default)]
    #[serde(rename = "enum")] pub enums: Vec<EnumXml>,
    #[serde(default)]
    #[serde(rename = "view")] pub views: Vec<ViewXml>,
    #[serde(default)]
    #[serde(rename = "functions")] pub functions: Vec<FunctionsXml>,
    #[serde(default)]
    #[serde(rename = "procedures")] pub procedures: Vec<ProceduresXml>,
    #[serde(default)]
    #[serde(rename = "otherSql")] pub other_sql: Vec<OtherSqlXml>,
    #[serde(default)]
    #[serde(rename = "customSQL")] pub custom_sql: Vec<CustomSqlXml>,

    #[serde(default)]
    #[serde(rename = "schema")] pub schemas: Vec<SchemaXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchemaXml {
    #[serde(rename = "@name")] pub name: String,
    #[serde(default)]
    #[serde(rename = "table")] pub tables: Vec<TableXml>,
    #[serde(default)]
    #[serde(rename = "enum")] pub enums: Vec<EnumXml>,
    #[serde(default)]
    #[serde(rename = "view")] pub views: Vec<ViewXml>,
    #[serde(default)]
    #[serde(rename = "functions")] pub functions: Vec<FunctionsXml>,
    #[serde(default)]
    #[serde(rename = "procedures")] pub procedures: Vec<ProceduresXml>,
    #[serde(default)]
    #[serde(rename = "otherSql")] pub other_sql: Vec<OtherSqlXml>,
    #[serde(default)]
    #[serde(rename = "customSQL")] pub custom_sql: Vec<CustomSqlXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableXml {
    #[serde(rename = "@name")] pub name: String,
    #[serde(rename = "@data")] pub data_opt: Option<bool>,
    #[serde(rename = "@noExport")] pub no_export: Option<bool>,
    #[serde(rename = "@exportDataColumn")] pub export_data_column: Option<String>,
    #[serde(rename = "@compress")] pub compress: Option<bool>,
    #[serde(rename = "@lockEscalation")] pub lock_escalation: Option<String>,

    #[serde(rename = "columns")] pub columns: Option<ColumnsXml>,
    #[serde(rename = "keys")] pub keys: Option<KeysXml>,
    #[serde(rename = "relations")] pub relations: Option<RelationsXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnsXml { #[serde(rename = "column")] pub column: Vec<ColumnXml> }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnXml {
    #[serde(rename = "@name")] pub name: String,
    #[serde(rename = "@type")] pub r#type: String,
    #[serde(rename = "@length")] pub length: Option<i32>,
    #[serde(rename = "@scale")] pub scale: Option<i32>,
    #[serde(rename = "@required")] pub required: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeysXml {
    #[serde(rename = "primary")] pub primary: Option<KeyColumnsXml>,
    #[serde(default)]
    #[serde(rename = "unique")] pub uniques: Vec<KeyColumnsXml>,
    #[serde(default)]
    #[serde(rename = "index")] pub indexes: Vec<IndexXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyColumnsXml { #[serde(rename = "column")] pub columns: Vec<KeyColumnXml> }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexXml { #[serde(rename = "column")] pub columns: Vec<KeyColumnXml> }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyColumnXml { #[serde(rename = "@name")] pub name: String }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationsXml { #[serde(rename = "relation")] pub relation: Vec<RelationXml> }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationXml {
    #[serde(rename = "@src")] pub src: String,
    #[serde(rename = "@table")] pub table: String,
    #[serde(rename = "@column")] pub column: String,
    #[serde(rename = "@type")] pub r#type: String,
    #[serde(rename = "@disableUsageChecking")] pub disable_usage_checking: Option<bool>,
}

// Minimal stubs to satisfy the top-level shapes; not converted yet
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ViewXml { #[serde(rename = "@name")] pub name: String, #[serde(rename = "@databaseType")] pub database_type: Option<String>, #[serde(rename = "$text")] pub sql: String }
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionsXml {}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProceduresXml {}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtherSqlXml {}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomSqlXml {}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnumXml { #[serde(rename = "@name")] pub name: String, }
