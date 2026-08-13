use crate::error::SchemaReverseEngineerError;
use sqlx::PgPool;

pub async fn list_tables(pool: &PgPool, db_schema: &str) -> Result<Vec<String>, SchemaReverseEngineerError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name \
         FROM information_schema.tables \
         WHERE table_schema = $1 AND table_type = 'BASE TABLE' \
         ORDER BY table_name",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}
