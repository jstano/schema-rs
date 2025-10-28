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

    #[serde(default)]
    #[serde(rename = "table")]
    pub tables: Vec<TableXml>,

    #[serde(default)]
    #[serde(rename = "enum")]
    pub enums: Vec<EnumXml>,

    #[serde(default)]
    #[serde(rename = "view")]
    pub views: Vec<ViewXml>,

    #[serde(default)]
    #[serde(rename = "functions")]
    pub functions: Vec<FunctionsXml>,

    #[serde(default)]
    #[serde(rename = "procedures")]
    pub procedures: Vec<ProceduresXml>,

    #[serde(default)]
    #[serde(rename = "otherSql")]
    pub other_sql: Vec<OtherSqlXml>,

    #[serde(default)]
    #[serde(rename = "customSQL")]
    pub custom_sql: Vec<CustomSqlXml>,

    #[serde(default)]
    #[serde(rename = "schema")]
    pub schemas: Vec<SchemaXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchemaXml {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(default)]
    #[serde(rename = "table")]
    pub tables: Vec<TableXml>,

    #[serde(default)]
    #[serde(rename = "enum")]
    pub enums: Vec<EnumXml>,

    #[serde(default)]
    #[serde(rename = "view")]
    pub views: Vec<ViewXml>,

    #[serde(default)]
    #[serde(rename = "functions")]
    pub functions: Vec<FunctionsXml>,

    #[serde(default)]
    #[serde(rename = "procedures")]
    pub procedures: Vec<ProceduresXml>,

    #[serde(default)]
    #[serde(rename = "otherSql")]
    pub other_sql: Vec<OtherSqlXml>,

    #[serde(default)]
    #[serde(rename = "customSQL")]
    pub custom_sql: Vec<CustomSqlXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@data")]
    pub data_opt: Option<bool>,
    #[serde(rename = "@noExport")]
    pub no_export: Option<bool>,
    #[serde(rename = "@exportDataColumn")]
    pub export_data_column: Option<String>,
    #[serde(rename = "@compress")]
    pub compress: Option<bool>,
    #[serde(rename = "@lockEscalation")]
    pub lock_escalation: Option<String>,
    #[serde(rename = "columns")]
    pub columns: Option<ColumnsXml>,
    #[serde(rename = "keys")]
    pub keys: Option<KeysXml>,
    #[serde(rename = "relations")]
    pub relations: Option<RelationsXml>,
    #[serde(rename = "triggers")]
    pub triggers: Option<TriggersXml>,
    #[serde(rename = "constraints")]
    pub constraints: Option<ConstraintsXml>,
    #[serde(rename = "aggregations")]
    pub aggregations: Option<AggregationsXml>,
    #[serde(rename = "initialData")]
    pub initial_data: Option<InitialDataXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnsXml {
    #[serde(rename = "column")]
    pub column: Vec<ColumnXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@length")]
    pub length: Option<i32>,
    #[serde(rename = "@scale")]
    pub scale: Option<i32>,
    #[serde(rename = "@required")]
    pub required: Option<bool>,
    #[serde(rename = "@unicode")]
    pub unicode: Option<bool>,
    #[serde(rename = "@ignoreCase")]
    pub ignore_case: Option<bool>,
    #[serde(rename = "@default")]
    pub default_value: Option<String>,
    #[serde(rename = "@generated")]
    pub generated: Option<String>,
    #[serde(rename = "@enumType")]
    pub enum_type: Option<String>,
    #[serde(rename = "@elementType")]
    pub element_type: Option<String>,
    #[serde(rename = "@minValue")]
    pub min_value: Option<f64>,
    #[serde(rename = "@maxValue")]
    pub max_value: Option<f64>,
    #[serde(rename = "check")]
    pub check: Option<CheckXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeysXml {
    #[serde(rename = "primary")]
    pub primary: Option<KeyColumnsXml>,
    #[serde(default)]
    #[serde(rename = "unique")]
    pub uniques: Vec<KeyColumnsXml>,
    #[serde(default)]
    #[serde(rename = "index")]
    pub indexes: Vec<IndexXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyColumnsXml {
    #[serde(rename = "column")]
    pub columns: Vec<KeyColumnXml>,
    #[serde(rename = "@cluster")]
    pub cluster: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexXml {
    #[serde(rename = "column")]
    pub columns: Vec<KeyColumnXml>,
    #[serde(rename = "@include")]
    pub include: Option<String>,
    #[serde(rename = "@compress")]
    pub compress: Option<bool>,
    #[serde(rename = "@unique")]
    pub unique: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyColumnXml {
    #[serde(rename = "@name")]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationsXml {
    #[serde(rename = "relation")]
    pub relation: Vec<RelationXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationXml {
    #[serde(rename = "@src")]
    pub src: String,
    #[serde(rename = "@table")]
    pub table: String,
    #[serde(rename = "@column")]
    pub column: String,
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@disableUsageChecking")]
    pub disable_usage_checking: Option<bool>,
}

// Minimal stubs to satisfy the top-level shapes; not converted yet
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ViewXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@databaseType")]
    pub database_type: Option<String>,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionsXml {
    #[serde(rename = "function")]
    pub function: Vec<FunctionXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "sql")]
    pub sql: Vec<VendorSqlXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VendorSqlXml {
    #[serde(rename = "@databaseType")]
    pub database_type: String,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProceduresXml {
    #[serde(rename = "procedure")]
    pub procedure: Vec<ProcedureXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProcedureXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "sql")]
    pub sql: Vec<VendorSqlXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum OtherSqlOrderXml {
    #[serde(rename = "top")]
    Top,
    #[serde(rename = "bottom")]
    Bottom,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtherSqlXml {
    #[serde(rename = "@databaseType")]
    pub database_type: String,
    #[serde(rename = "@order")]
    pub order: OtherSqlOrderXml,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomSqlXml {
    #[serde(rename = "@databaseType")]
    pub database_type: String,
    #[serde(default)]
    #[serde(rename = "function")]
    pub function: Vec<LegacyFunctionXml>,
    #[serde(default)]
    #[serde(rename = "procedure")]
    pub procedure: Vec<LegacyProcedureXml>,
    #[serde(rename = "other")]
    pub other: Option<OtherXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegacyFunctionXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegacyProcedureXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtherXml {
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnumXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "value")]
    pub value: Vec<EnumValueXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnumValueXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@code")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CheckXml {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TriggersXml {
    #[serde(default)]
    #[serde(rename = "update")]
    pub update: Vec<TriggerXml>,
    #[serde(default)]
    #[serde(rename = "delete")]
    pub delete: Vec<TriggerXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TriggerXml {
    #[serde(rename = "@databaseType")]
    pub database_type: String,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstraintsXml {
    #[serde(rename = "constraint")]
    pub constraint: Vec<ConstraintXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstraintXml {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@databaseType")]
    pub database_type: Option<String>,
    #[serde(rename = "$text")]
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AggregationsXml {
    #[serde(rename = "aggregate")]
    pub aggregate: Vec<AggregateXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AggregateXml {
    #[serde(default)]
    #[serde(rename = "sum")]
    pub sum: Vec<SumXml>,
    #[serde(default)]
    #[serde(rename = "count")]
    pub count: Vec<CountXml>,
    #[serde(rename = "group")]
    pub group: GroupXml,
    #[serde(rename = "@destinationTable")]
    pub destination_table: String,
    #[serde(rename = "@dateColumn")]
    pub date_column: String,
    #[serde(rename = "@timestampColumn")]
    pub timestamp_column: String,
    #[serde(rename = "@frequency")]
    pub frequency: String,
    #[serde(rename = "@criteria")]
    pub criteria: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SumXml {
    #[serde(rename = "@sourceColumn")]
    pub source_column: String,
    #[serde(rename = "@destinationColumn")]
    pub destination_column: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CountXml {
    #[serde(rename = "@destinationColumn")]
    pub destination_column: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupXml {
    #[serde(rename = "column")]
    pub column: Vec<GroupColumnXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroupColumnXml {
    #[serde(rename = "@source")]
    pub source: String,
    #[serde(rename = "@destination")]
    pub destination: String,
    #[serde(rename = "@sourceDerivedFrom")]
    pub source_derived_from: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitialDataXml {
    #[serde(rename = "sql")]
    pub sql: Vec<InitialSqlXml>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitialSqlXml {
    #[serde(rename = "@databaseType")]
    pub database_type: Option<String>,
    #[serde(rename = "$text")]
    pub sql: String,
}
