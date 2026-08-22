use super::nodes::*;
use crate::parser::table_parser::parse_table;
use schema_model::builder::SchemaBuilder;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::relation::Relation;
use schema_model::model::schema::Schema;
use schema_model::model::types::{
    BooleanMode, DatabaseType, ForeignKeyMode, OtherSqlOrder,
};
use schema_model::model::view::View;
use schema_model::model::{
    aggregation::AggregationFrequency,
    enum_type::{EnumType, EnumValue},
    function::Function,
    other_sql::OtherSql,
    procedure::Procedure,
};

pub fn convert_database(database_xml: DatabaseXml) -> Result<DatabaseModel, String> {
    let mut schemas: Vec<Schema> = Vec::new();

    if let Some(default_schema) = default_schema(&database_xml)? {
        schemas.push(default_schema);
    }

    for schema_xml in database_xml.schemas.into_iter() {
        schemas.push(sub_schema(&schema_xml)?);
    }

    let boolean_mode = database_xml
        .boolean_mode
        .as_deref()
        .map(|s| s.parse::<BooleanMode>())
        .unwrap_or(Ok(BooleanMode::Native))
        .unwrap();
    let foreign_key_mode = database_xml
        .foreign_key_mode
        .as_deref()
        .map(|s| s.parse::<ForeignKeyMode>())
        .unwrap_or(Ok(ForeignKeyMode::Relations))
        .unwrap();

    let mut database_model = DatabaseModel::new(boolean_mode, foreign_key_mode, schemas);

    database_model.sort_tables_by_name();
    reverse_relations(&mut database_model)?;

    Ok(database_model)
}

fn default_schema(database: &DatabaseXml) -> Result<Option<Schema>, String> {
    let mut schema_builder = SchemaBuilder::new(None::<&str>);

    if let Some(cst) = database.case_sensitive_text {
        schema_builder = schema_builder.case_sensitive_text(cst);
    }

    for table_xml in database.tables.iter() {
        let table = parse_table(table_xml, None)?;
        schema_builder = schema_builder.add_table(table);
    }

    for view_xml in database.views.iter() {
        let database_type = optional_database_type(
            view_xml.database_type.as_deref(),
            &format!("view '{}'", view_xml.name),
        )?;
        schema_builder = schema_builder.add_view(View::new(
            None,
            &view_xml.name,
            &view_xml.sql,
            database_type,
        ));
    }

    for enum_xml in database.enums.iter() {
        let evs: Vec<EnumValue> = enum_xml
            .value
            .iter()
            .map(|v| EnumValue::new(&v.name, v.code.clone()))
            .collect();
        schema_builder = schema_builder.add_enum_type(EnumType::new(&enum_xml.name, evs));
    }

    for function_xml in database.functions.iter() {
        let mut functions: Vec<Function> = Vec::new();
        for vendor_sql_xml in function_xml.sql.iter() {
            let database_type = required_database_type(
                Some(&vendor_sql_xml.database_type),
                &format!("function '{}'", function_xml.name),
            )?;
            functions.push(Function::new(
                None,
                &function_xml.name,
                database_type,
                &vendor_sql_xml.sql,
            ));
        }
        if !functions.is_empty() {
            schema_builder = schema_builder.add_functions(functions);
        }
    }

    for procedure_xml in database.procedures.iter() {
        let mut procedures: Vec<Procedure> = Vec::new();
        for vendor_sql_xml in procedure_xml.sql.iter() {
            let database_type = required_database_type(
                Some(&vendor_sql_xml.database_type),
                &format!("procedure '{}'", procedure_xml.name),
            )?;
            procedures.push(Procedure::new(
                None,
                &procedure_xml.name,
                database_type,
                &vendor_sql_xml.sql,
            ));
        }
        if !procedures.is_empty() {
            schema_builder = schema_builder.add_procedures(procedures);
        }
    }

    for other_sql_xml in database.other_sql.iter() {
        let database_type = required_database_type(Some(&other_sql_xml.database_type), "otherSql")?;
        if let Some(order) = other_sql_order(&other_sql_xml.order) {
            schema_builder = schema_builder.add_other_sql(OtherSql::new(
                database_type,
                order,
                &other_sql_xml.sql,
            ));
        }
    }

    let root_schema = schema_builder.build();
    if !root_schema.tables().is_empty()
        || !database.views.is_empty()
        || !database.enums.is_empty()
        || !database.functions.is_empty()
        || !database.procedures.is_empty()
        || !database.other_sql.is_empty()
    {
        return Ok(Some(root_schema));
    }

    Ok(None)
}

fn sub_schema(schema_xml: &SchemaXml) -> Result<Schema, String> {
    let mut schema_builder = SchemaBuilder::new(Some(&schema_xml.name));

    if let Some(cst) = schema_xml.case_sensitive_text {
        schema_builder = schema_builder.case_sensitive_text(cst);
    }

    for table_xml in schema_xml.tables.iter() {
        schema_builder = schema_builder.add_table(parse_table(table_xml, Some(&schema_xml.name))?);
    }

    for view_xml in schema_xml.views.iter() {
        let database_type = optional_database_type(
            view_xml.database_type.as_deref(),
            &format!("view '{}.{}'", schema_xml.name, view_xml.name),
        )?;
        schema_builder = schema_builder.add_view(View::new(
            Some(&schema_xml.name),
            &view_xml.name,
            &view_xml.sql,
            database_type,
        ));
    }

    for enum_xml in schema_xml.enums.iter() {
        let enum_values: Vec<EnumValue> = enum_xml
            .value
            .iter()
            .map(|v| EnumValue::new(&v.name, v.code.clone()))
            .collect();
        schema_builder = schema_builder.add_enum_type(EnumType::new(&enum_xml.name, enum_values));
    }

    for function_xml in schema_xml.functions.iter() {
        let mut functions: Vec<Function> = Vec::new();
        for vendor_sql_xml in function_xml.sql.iter() {
            let database_type = required_database_type(
                Some(&vendor_sql_xml.database_type),
                &format!("function '{}.{}'", schema_xml.name, function_xml.name),
            )?;
            functions.push(Function::new(
                Some(schema_xml.name.as_str()),
                &function_xml.name,
                database_type,
                &vendor_sql_xml.sql,
            ));
        }
        if !functions.is_empty() {
            schema_builder = schema_builder.add_functions(functions);
        }
    }

    for procedure_xml in schema_xml.procedures.iter() {
        let mut procedures: Vec<Procedure> = Vec::new();
        for vendor_sql_xml in procedure_xml.sql.iter() {
            let database_type = required_database_type(
                Some(&vendor_sql_xml.database_type),
                &format!("procedure '{}.{}'", schema_xml.name, procedure_xml.name),
            )?;
            procedures.push(Procedure::new(
                Some(schema_xml.name.as_str()),
                &procedure_xml.name,
                database_type,
                &vendor_sql_xml.sql,
            ));
        }
        if !procedures.is_empty() {
            schema_builder = schema_builder.add_procedures(procedures);
        }
    }

    for other_sql_xml in schema_xml.other_sql.iter() {
        let database_type = required_database_type(
            Some(&other_sql_xml.database_type),
            &format!("otherSql in schema '{}'", schema_xml.name),
        )?;
        if let Some(order) = other_sql_order(&other_sql_xml.order) {
            schema_builder = schema_builder.add_other_sql(OtherSql::new(
                database_type,
                order,
                &other_sql_xml.sql,
            ));
        }
    }

    Ok(schema_builder.build())
}

pub(crate) fn str_to_database_type(s: Option<&str>) -> Option<DatabaseType> {
    s.and_then(|v| match v.to_ascii_lowercase().as_str() {
        "postgresql" => Some(DatabaseType::Postgresql),
        "sqlite" => Some(DatabaseType::Sqlite),
        "sqlserver" | "mssql" => Some(DatabaseType::SqlServer),
        _ => None,
    })
}

/// Parses a `databaseType` attribute that's required for the element to mean anything
/// (functions/procedures/otherSql/triggers/constraints all need one specific target
/// database - none of them can represent "applies to every database"). Returns an error
/// rather than silently discarding the element when the attribute is missing or doesn't
/// match a known database type; a typo here used to make the whole element vanish from
/// the parsed model with no warning.
pub(crate) fn required_database_type(value: Option<&str>, context: &str) -> Result<DatabaseType, String> {
    let value = value.ok_or_else(|| format!("{context}: missing required databaseType attribute"))?;
    str_to_database_type(Some(value)).ok_or_else(|| format!("{context}: unrecognized databaseType '{value}'"))
}

/// Parses an optional `databaseType` attribute where absence has a real meaning: "applies
/// to every database type" (mirrors `View`/`InitialData`'s `Option<DatabaseType>`). An
/// attribute that IS present but unrecognized is still always an error, so a typo never
/// silently broadens an element's scope to every database instead of the one intended.
pub(crate) fn optional_database_type(value: Option<&str>, context: &str) -> Result<Option<DatabaseType>, String> {
    match value {
        None => Ok(None),
        Some(v) => str_to_database_type(Some(v))
            .map(Some)
            .ok_or_else(|| format!("{context}: unrecognized databaseType '{v}'")),
    }
}

fn other_sql_order(o: &OtherSqlOrderXml) -> Option<OtherSqlOrder> {
    match o {
        OtherSqlOrderXml::Top => Some(OtherSqlOrder::Top),
        OtherSqlOrderXml::Bottom => Some(OtherSqlOrder::Bottom),
    }
}

pub(crate) fn agg_frequency_from_str(s: &str) -> Result<AggregationFrequency, String> {
    match s.to_ascii_lowercase().as_str() {
        "daily" => Ok(AggregationFrequency::Daily),
        "weekly" => Ok(AggregationFrequency::Weekly),
        "monthly" => Ok(AggregationFrequency::Monthly),
        "yearly" => Ok(AggregationFrequency::Yearly),
        other => Err(format!(
            "unrecognized frequency '{other}' (expected daily, weekly, monthly, or yearly)"
        )),
    }
}

fn reverse_relations(database_model: &mut DatabaseModel) -> Result<(), String> {
    // First pass: collect all reverse relation updates
    let mut updates = Vec::new();

    for table in database_model.all_tables() {
        for relation in table.relations() {
            let parent_table_name = relation.to_table_name();
            let parent_table_parts = split_schema_table(parent_table_name);

            // `relation.from_table_name()` is just `table`'s own bare name (see
            // `parse_relations` in table_parser.rs, which has no schema context) -
            // schema-qualify it here using `table`'s own schema, the same way
            // `to_table_name` is already schema-qualified by the XML author, so the
            // reverse relation stored on the parent can resolve the child table
            // correctly (via `find_table_by_qualified_name`) even when the child
            // lives in a non-default schema.
            let qualified_from_table_name = qualify_table_name(table.schema_name(), relation.from_table_name());

            updates.push((
                parent_table_parts.0,
                parent_table_parts.1.to_string(),
                qualified_from_table_name.clone(),
                Relation::new(
                    relation.to_table_name(),
                    relation.to_column_name(),
                    qualified_from_table_name.as_str(),
                    relation.from_column_name(),
                    relation.relation_type(),
                    false,
                ),
            ));
        }
    }

    // Second pass: apply updates using mutable borrows. A relation whose target table
    // doesn't exist (typo'd name, or a schema that was never declared) is a malformed
    // schema definition, not a bug in this code, so it's surfaced as an `Err` rather
    // than panicking the whole parse.
    for (schema, table_name, from_table_name, reverse_relation) in updates {
        let parent_table = database_model
            .find_table_mut_checked(schema.as_deref(), &table_name)
            .ok_or_else(|| {
                format!(
                    "table '{}' has a relation to '{}' which does not exist",
                    from_table_name,
                    schema
                        .as_deref()
                        .map(|s| format!("{}.{}", s, table_name))
                        .unwrap_or_else(|| table_name.clone())
                )
            })?;
        parent_table.add_reverse_relation(reverse_relation);
    }

    Ok(())
}

/// Renders `table_name` as `schema.table_name` when `schema_name` is present, or bare
/// `table_name` otherwise - matching the format `find_table_by_qualified_name` expects
/// (an unqualified name resolves to the default schema).
fn qualify_table_name(schema_name: Option<&str>, table_name: &str) -> String {
    match schema_name {
        Some(schema) => format!("{}.{}", schema, table_name),
        None => table_name.to_string(),
    }
}

fn split_schema_table(table_name: &str) -> (Option<String>, String) {
    if let Some(pos) = table_name.find('.') {
        let schema = table_name[..pos].to_string();
        let table = table_name[pos + 1..].to_string();
        (Some(schema), table)
    } else {
        (None, table_name.to_string())
    }
}
