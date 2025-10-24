use schema_model::builder::{ColumnBuilder, KeyBuilder, SchemaBuilder, TableBuilder};
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::relation::Relation;
use schema_model::model::schema::Schema;
use schema_model::model::types::{KeyType, LockEscalation, RelationType, Version};
use schema_model::model::view::View;

use super::nodes::*;

pub fn convert_database(db: DatabaseXml) -> Result<DatabaseModel, String> {
    let mut schemas: Vec<Schema> = Vec::new();

    // Root schema "" using builder
    let mut root_builder = SchemaBuilder::new("");

    for t in db.tables.iter() {
        let tbl = convert_table(t, "")?;
        root_builder = root_builder.add_table(tbl);
    }

    for v in db.views.iter() {
        if let Some(dt) = str_to_database_type_opt(v.database_type.as_deref()) {
            root_builder = root_builder.add_view(View::new("", &v.name, &v.sql, dt));
        }
    }

    let root_schema = root_builder.build();
    if !root_schema.tables().is_empty() || !db.views.is_empty() {
        schemas.push(root_schema);
    }

    for s in db.schemas.into_iter() {
        let mut sb = SchemaBuilder::new(&s.name);
        for t in s.tables.iter() {
            sb = sb.add_table(convert_table(t, &s.name)?);
        }
        for v in s.views.iter() {
            if let Some(dt) = str_to_database_type_opt(v.database_type.as_deref()) {
                sb = sb.add_view(View::new(&s.name, &v.name, &v.sql, dt));
            }
        }
        schemas.push(sb.build());
    }

    let version = if db.version.is_some() {
        Some(Version::parse(db.version.as_deref().unwrap_or("")))
    } else {
        None
    };

    Ok(DatabaseModel::new(
        version,
        schemas,
    ))
}

fn convert_table(
    t: &TableXml,
    schema_name: &str,
) -> Result<schema_model::model::table::Table, String> {
    let le = match t.lock_escalation.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "auto" => LockEscalation::Auto,
        _ => LockEscalation::Auto,
    };

    let mut tb = TableBuilder::new(schema_name, &t.name).no_export(t.no_export.unwrap_or(false));
    if let Some(c) = &t.export_data_column {
        tb = tb.export_date_column(c.clone());
    }
    tb = tb.lock_escalation(le);

    if let Some(cols) = &t.columns {
        for c in cols.column.iter() {
            let ctype = ColumnType::from_type_name(&c.r#type)
                .map_err(|e| format!("column '{}' type error: {}", c.name, e))?;
            let col = ColumnBuilder::new(&c.name, ctype)
                .length(c.length.unwrap_or(0))
                .scale(c.scale.unwrap_or(0))
                .required(c.required.unwrap_or(false))
                .build();
            tb = tb.add_column(col);
        }
    }

    if let Some(keys) = &t.keys {
        if let Some(pk) = &keys.primary {
            let mut kb = KeyBuilder::new(KeyType::Primary);
            for kc in pk.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            tb = tb.add_key(kb.build());
        }
        for uq in keys.uniques.iter() {
            let mut kb = KeyBuilder::new(KeyType::Unique);
            for kc in uq.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            tb = tb.add_key(kb.build());
        }
        for idx in keys.indexes.iter() {
            let mut kb = KeyBuilder::new(KeyType::Index);
            for kc in idx.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            tb = tb.add_index(kb.build());
        }
    }

    if let Some(rels) = &t.relations {
        for r in rels.relation.iter() {
            let rtype = match r.r#type.to_ascii_lowercase().as_str() {
                "cascade" => RelationType::Cascade,
                "enforce" => RelationType::Enforce,
                "setnull" => RelationType::SetNull,
                "donothing" => RelationType::DoNothing,
                _ => RelationType::Enforce,
            };
            tb = tb.add_relation(Relation::new(
                r.table.clone(),
                r.column.clone(),
                t.name.clone(),
                r.src.clone(),
                rtype,
                r.disable_usage_checking.unwrap_or(false),
            ));
        }
    }

    Ok(tb.build())
}

fn str_to_database_type_opt(s: Option<&str>) -> Option<schema_model::model::types::DatabaseType> {
    s.and_then(|v| match v.to_ascii_lowercase().as_str() {
        "postgresql" | "postgres" => {
            Some(schema_model::model::types::DatabaseType::Postgres)
        }
        "mysql" => Some(schema_model::model::types::DatabaseType::Mysql),
        "sqlite" => Some(schema_model::model::types::DatabaseType::Sqlite),
        "sqlserver" => Some(schema_model::model::types::DatabaseType::SqlServer),
        _ => None,
    })
}
