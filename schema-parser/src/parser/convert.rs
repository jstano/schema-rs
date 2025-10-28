use schema_model::builder::SchemaBuilder;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::schema::Schema;
use schema_model::model::types::{
    DatabaseType, OtherSqlOrder, Version,
};
use schema_model::model::view::View;
use schema_model::model::{
    aggregation::AggregationFrequency
    ,
    enum_type::{EnumType, EnumValue},
    function::Function
    ,
    other_sql::OtherSql,
    procedure::Procedure
    ,
};
use crate::parser::table_parser::parse_table;
use super::nodes::*;

pub fn convert_database(database: DatabaseXml) -> Result<DatabaseModel, String> {
    let mut schemas: Vec<Schema> = Vec::new();

    if let Some(root_schema) = root_schema(&database) {
        schemas.push(root_schema);
    }

    for schemaXml in database.schemas.into_iter() {
        if let Some(s) = sub_schema(&schemaXml) {
            schemas.push(s);
        }
    }

    let version = if database.version.is_some() {
        Some(Version::parse(database.version.as_deref().unwrap_or("")))
    } else {
        None
    };

    Ok(DatabaseModel::new(
        version,
        schemas,
    ))
}

fn root_schema(database: &DatabaseXml) -> Option<Schema> {
    let mut root_builder = SchemaBuilder::new(None::<&str>);

    for table_xml in database.tables.iter() {
        let table = parse_table(table_xml, None);
        root_builder = root_builder.add_table(table);
    }

    for view_xml in database.views.iter() {
        if let Some(database_type) = str_to_database_type_opt(view_xml.database_type.as_deref()) {
            root_builder = root_builder.add_view(View::new(None, &view_xml.name, &view_xml.sql, database_type));
        }
    }

    // Root-level enums
    for enum_xml in database.enums.iter() {
        let evs: Vec<EnumValue> = enum_xml
            .value
            .iter()
            .map(|v| EnumValue::new(&v.name, v.code.clone()))
            .collect();
        root_builder = root_builder.add_enum_type(EnumType::new(&enum_xml.name, evs));
    }

    for functions_xml in database.functions.iter() {
        let mut functions: Vec<Function> = Vec::new();
        for function_xml in functions_xml.function.iter() {
            for sql in function_xml.sql.iter() {
                if let Some(database_type) = str_to_database_type_opt(Some(&sql.database_type)) {
                    functions.push(Function::new("", &function_xml.name, database_type, &sql.sql));
                }
            }
        }
        if !functions.is_empty() {
            root_builder = root_builder.add_functions(functions);
        }
    }

    for procedure_xmls in database.procedures.iter() {
        let mut procedures: Vec<Procedure> = Vec::new();
        for procedure_xml in procedure_xmls.procedure.iter() {
            for sql in procedure_xml.sql.iter() {
                if let Some(database_type) = str_to_database_type_opt(Some(&sql.database_type)) {
                    procedures.push(Procedure::new("", &procedure_xml.name, database_type, &sql.sql));
                }
            }
        }
        if !procedures.is_empty() {
            root_builder = root_builder.add_procedures(procedures);
        }
    }

    for other_sql_xml in database.other_sql.iter() {
        if let (Some(database_type), Some(order)) = (
            str_to_database_type_opt(Some(&other_sql_xml.database_type)),
            other_sql_order(&other_sql_xml.order),
        ) {
            root_builder = root_builder.add_other_sql(OtherSql::new(database_type, order, &other_sql_xml.sql));
        }
    }

    for custom_sql_xml in database.custom_sql.iter() {
        if let Some(database_type) = str_to_database_type_opt(Some(&custom_sql_xml.database_type)) {
            let mut functions: Vec<Function> = Vec::new();
            for function_xml in custom_sql_xml.function.iter() {
                functions.push(Function::new("", &function_xml.name, database_type, &function_xml.sql));
            }
            if !functions.is_empty() {
                root_builder = root_builder.add_functions(functions);
            }

            let mut procedures: Vec<Procedure> = Vec::new();
            for procedure_xml in custom_sql_xml.procedure.iter() {
                procedures.push(Procedure::new("", &procedure_xml.name, database_type, &procedure_xml.sql));
            }
            if !procedures.is_empty() {
                root_builder = root_builder.add_procedures(procedures);
            }
            // cs.other has no order attribute; no clear mapping into OtherSql → skip
        }
    }

    let root_schema = root_builder.build();
    if !root_schema.tables().is_empty()
        || !database.views.is_empty()
        || !database.enums.is_empty()
        || !database.functions.is_empty()
        || !database.procedures.is_empty()
        || !database.other_sql.is_empty()
        || !database.custom_sql.is_empty()
    {
        return Some(root_schema)
    }

    None
}

fn sub_schema(schema_xml: &SchemaXml) -> Option<Schema> {
    let mut schema_builder = SchemaBuilder::new(Some(&schema_xml.name));

    for table_xml in schema_xml.tables.iter() {
        schema_builder = schema_builder.add_table(parse_table(table_xml, Some(&schema_xml.name)));
    }

    for view_xml in schema_xml.views.iter() {
        if let Some(dt) = str_to_database_type_opt(view_xml.database_type.as_deref()) {
            schema_builder = schema_builder.add_view(View::new(Some(&schema_xml.name), &view_xml.name, &view_xml.sql, dt));
        }
    }

    for enum_xml in schema_xml.enums.iter() {
        let evs: Vec<EnumValue> = enum_xml
            .value
            .iter()
            .map(|v| EnumValue::new(&v.name, v.code.clone()))
            .collect();
        schema_builder = schema_builder.add_enum_type(EnumType::new(&enum_xml.name, evs));
    }

    for functions_xml in schema_xml.functions.iter() {
        let mut functions: Vec<Function> = Vec::new();
        for f in functions_xml.function.iter() {
            for sql in f.sql.iter() {
                if let Some(dt) = str_to_database_type_opt(Some(&sql.database_type)) {
                    functions.push(Function::new(&schema_xml.name, &f.name, dt, &sql.sql));
                }
            }
        }
        if !functions.is_empty() {
            schema_builder = schema_builder.add_functions(functions);
        }
    }

    for procedures_xml in schema_xml.procedures.iter() {
        let mut procedures: Vec<Procedure> = Vec::new();
        for p in procedures_xml.procedure.iter() {
            for sql in p.sql.iter() {
                if let Some(dt) = str_to_database_type_opt(Some(&sql.database_type)) {
                    procedures.push(Procedure::new(&schema_xml.name, &p.name, dt, &sql.sql));
                }
            }
        }
        if !procedures.is_empty() {
            schema_builder = schema_builder.add_procedures(procedures);
        }
    }

    for other_sql_xml in schema_xml.other_sql.iter() {
        if let (Some(dt), Some(order)) = (
            str_to_database_type_opt(Some(&other_sql_xml.database_type)),
            other_sql_order(&other_sql_xml.order),
        ) {
            schema_builder = schema_builder.add_other_sql(OtherSql::new(dt, order, &other_sql_xml.sql));
        }
    }

    for custom_sql_xml in schema_xml.custom_sql.iter() {
        if let Some(dt) = str_to_database_type_opt(Some(&custom_sql_xml.database_type)) {
            let mut functions: Vec<Function> = Vec::new();
            for f in custom_sql_xml.function.iter() {
                functions.push(Function::new(&schema_xml.name, &f.name, dt, &f.sql));
            }
            if !functions.is_empty() {
                schema_builder = schema_builder.add_functions(functions);
            }
            let mut procedures: Vec<Procedure> = Vec::new();
            for p in custom_sql_xml.procedure.iter() {
                procedures.push(Procedure::new(&schema_xml.name, &p.name, dt, &p.sql));
            }
            if !procedures.is_empty() {
                schema_builder = schema_builder.add_procedures(procedures);
            }
        }
    }

    Some(schema_builder.build())
}

pub(crate) fn str_to_database_type_opt(s: Option<&str>) -> Option<schema_model::model::types::DatabaseType> {
    s.and_then(|v| match v.to_ascii_lowercase().as_str() {
        "postgresql" | "postgres" | "pgsql" => Some(DatabaseType::Postgres),
        "mysql" => Some(DatabaseType::Mysql),
        "sqlite" => Some(DatabaseType::Sqlite),
        "sqlserver" | "mssql" => Some(DatabaseType::SqlServer),
        // Map derby/hsql to H2 as a closest supported engine in this model
        "derby" | "hsql" => Some(DatabaseType::H2),
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
