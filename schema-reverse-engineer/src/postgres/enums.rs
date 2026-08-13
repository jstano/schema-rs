use crate::error::SchemaReverseEngineerError;
use schema_model::model::enum_type::{EnumType, EnumValue};
use sqlx::PgPool;
use std::collections::BTreeMap;

/// Reads native Postgres enum types (`CREATE TYPE ... AS ENUM (...)`) defined in `db_schema`.
pub async fn list_enum_types(pool: &PgPool, db_schema: &str) -> Result<Vec<EnumType>, SchemaReverseEngineerError> {
    let rows: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT t.typname, e.enumlabel, e.enumsortorder::int4 \
         FROM pg_type t \
         JOIN pg_enum e ON e.enumtypid = t.oid \
         JOIN pg_namespace n ON n.oid = t.typnamespace \
         WHERE n.nspname = $1 \
         ORDER BY t.typname, e.enumsortorder",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    let mut by_name: BTreeMap<String, Vec<EnumValue>> = BTreeMap::new();
    for (type_name, value_name, _sort_order) in rows {
        by_name
            .entry(type_name)
            .or_default()
            .push(EnumValue::new(value_name, None::<String>));
    }

    Ok(by_name
        .into_iter()
        .map(|(name, values)| EnumType::new(name, values))
        .collect())
}
