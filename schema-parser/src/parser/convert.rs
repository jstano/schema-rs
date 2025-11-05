use std::collections::HashMap;
use super::nodes::*;
use crate::parser::table_parser::parse_table;
use schema_model::builder::SchemaBuilder;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::relation::Relation;
use schema_model::model::schema::Schema;
use schema_model::model::types::{
    BooleanMode, DatabaseType, ForeignKeyMode, OtherSqlOrder, Version,
};
use schema_model::model::view::View;
use schema_model::model::{
    aggregation::AggregationFrequency,
    database_model,
    enum_type::{EnumType, EnumValue},
    function::Function,
    other_sql::OtherSql,
    procedure::Procedure,
};

pub fn convert_database(database_xml: DatabaseXml) -> DatabaseModel {
    let mut schemas: Vec<Schema> = Vec::new();

    if let Some(default_schema) = default_schema(&database_xml) {
        schemas.push(default_schema);
    }

    for schemaXml in database_xml.schemas.into_iter() {
        if let Some(s) = sub_schema(&schemaXml) {
            schemas.push(s);
        }
    }

    let version = database_xml.version.as_deref().map(Version::parse);
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

    let mut database_model = DatabaseModel::new(version, boolean_mode, foreign_key_mode, schemas);

    reverse_relations(&mut database_model);

    database_model
}

fn default_schema(database: &DatabaseXml) -> Option<Schema> {
    let mut schema_builder = SchemaBuilder::new(None::<&str>);

    for table_xml in database.tables.iter() {
        let table = parse_table(table_xml, None);
        schema_builder = schema_builder.add_table(table);
    }

    for view_xml in database.views.iter() {
        let database_type = str_to_database_type(view_xml.database_type.as_deref());
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
            if let Some(database_type) =
                str_to_database_type(Some(&vendor_sql_xml.database_type))
            {
                functions.push(Function::new(
                    None,
                    &function_xml.name,
                    database_type,
                    &vendor_sql_xml.sql,
                ));
            }
        }
        if !functions.is_empty() {
            schema_builder = schema_builder.add_functions(functions);
        }
    }

    for procedure_xml in database.procedures.iter() {
        let mut procedures: Vec<Procedure> = Vec::new();
        for vendor_sql_xml in procedure_xml.sql.iter() {
            if let Some(database_type) =
                str_to_database_type(Some(&vendor_sql_xml.database_type))
            {
                procedures.push(Procedure::new(
                    None,
                    &procedure_xml.name,
                    database_type,
                    &vendor_sql_xml.sql,
                ));
            }
        }
        if !procedures.is_empty() {
            schema_builder = schema_builder.add_procedures(procedures);
        }
    }

    for other_sql_xml in database.other_sql.iter() {
        if let (Some(database_type), Some(order)) = (
            str_to_database_type(Some(&other_sql_xml.database_type)),
            other_sql_order(&other_sql_xml.order),
        ) {
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
        return Some(root_schema);
    }

    None
}

fn sub_schema(schema_xml: &SchemaXml) -> Option<Schema> {
    let mut schema_builder = SchemaBuilder::new(Some(&schema_xml.name));

    for table_xml in schema_xml.tables.iter() {
        schema_builder = schema_builder.add_table(parse_table(table_xml, Some(&schema_xml.name)));
    }

    for view_xml in schema_xml.views.iter() {
        let database_type = str_to_database_type(view_xml.database_type.as_deref());
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
            if let Some(database_type) =
                str_to_database_type(Some(&vendor_sql_xml.database_type))
            {
                functions.push(Function::new(
                    Some(schema_xml.name.as_str()),
                    &function_xml.name,
                    database_type,
                    &vendor_sql_xml.sql,
                ));
            }
        }
        if !functions.is_empty() {
            schema_builder = schema_builder.add_functions(functions);
        }
    }

    for procedure_xml in schema_xml.procedures.iter() {
        let mut procedures: Vec<Procedure> = Vec::new();
        for vendor_sql_xml in procedure_xml.sql.iter() {
            if let Some(database_type) =
                str_to_database_type(Some(&vendor_sql_xml.database_type))
            {
                procedures.push(Procedure::new(
                    Some(schema_xml.name.as_str()),
                    &procedure_xml.name,
                    database_type,
                    &vendor_sql_xml.sql,
                ));
            }
        }
        if !procedures.is_empty() {
            schema_builder = schema_builder.add_procedures(procedures);
        }
    }

    for other_sql_xml in schema_xml.other_sql.iter() {
        if let (Some(database_type), Some(order)) = (
            str_to_database_type(Some(&other_sql_xml.database_type)),
            other_sql_order(&other_sql_xml.order),
        ) {
            schema_builder = schema_builder.add_other_sql(OtherSql::new(
                database_type,
                order,
                &other_sql_xml.sql,
            ));
        }
    }

    Some(schema_builder.build())
}

pub(crate) fn str_to_database_type(s: Option<&str>) -> Option<DatabaseType> {
    s.and_then(|v| match v.to_ascii_lowercase().as_str() {
        "postgresql" | "postgres" | "pgsql" => Some(DatabaseType::Postgres),
        "mysql" => Some(DatabaseType::Mysql),
        "sqlite" => Some(DatabaseType::Sqlite),
        "sqlserver" | "mssql" => Some(DatabaseType::SqlServer),
        _ => None,
    })
}

fn other_sql_order(o: &OtherSqlOrderXml) -> Option<OtherSqlOrder> {
    match o {
        OtherSqlOrderXml::Top => Some(OtherSqlOrder::Top),
        OtherSqlOrderXml::Bottom => Some(OtherSqlOrder::Bottom),
    }
}

pub(crate) fn agg_frequency_from_str(s: &str) -> AggregationFrequency {
    match s.to_ascii_lowercase().as_str() {
        "daily" => AggregationFrequency::Daily,
        "weekly" => AggregationFrequency::Weekly,
        "monthly" => AggregationFrequency::Monthly,
        "yearly" => AggregationFrequency::Yearly,
        _ => AggregationFrequency::Monthly,
    }
}

fn reverse_relations(database_model: &mut DatabaseModel) {
    // First pass: collect all reverse relation updates
    let mut updates = Vec::new();

    for table in database_model.all_tables() {
        for relation in table.relations() {
            let parent_table_name = relation.to_table_name();
            let parent_table_parts = split_schema_table(&parent_table_name);

            updates.push((
                parent_table_parts.0,
                parent_table_parts.1.to_string(),
                Relation::new(
                    relation.to_table_name(),
                    relation.to_column_name(),
                    relation.from_table_name(),
                    relation.from_column_name(),
                    relation.relation_type(),
                    false,
                ),
            ));
        }
    }

    // Second pass: apply updates using mutable borrows
    for (schema, table_name, reverse_relation) in updates {
        let parent_table = database_model.find_table_mut(schema.as_deref(), &table_name);
        parent_table.add_reverse_relation(reverse_relation);
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
