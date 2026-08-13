use crate::error::SchemaReverseEngineerError;
use schema_model::model::types::DatabaseType;
use schema_model::model::view::View;
use sqlx::PgPool;

pub async fn list_views(pool: &PgPool, db_schema: &str) -> Result<Vec<View>, SchemaReverseEngineerError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, view_definition \
         FROM information_schema.views \
         WHERE table_schema = $1 \
         ORDER BY table_name",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(name, sql)| View::new(None, name.as_str(), sql.trim(), Some(DatabaseType::Postgresql)))
        .collect())
}
