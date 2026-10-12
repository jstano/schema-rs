use crate::error::SchemaReverseEngineerError;
use schema_model::model::types::DatabaseType;
use schema_model::model::view::View;
use sqlx::PgPool;

pub async fn list_views(
    pool: &PgPool,
    db_schema: &str,
) -> Result<Vec<View>, SchemaReverseEngineerError> {
    // `view_definition` is NULL for a role that doesn't own the view.
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT table_name, view_definition \
         FROM information_schema.views \
         WHERE table_schema = $1 \
         ORDER BY table_name",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    // `information_schema.views` omits materialized views, and the model has no such concept.
    let materialized: Vec<(String,)> = sqlx::query_as(
        "SELECT matviewname FROM pg_matviews WHERE schemaname = $1 ORDER BY matviewname",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;
    for (name,) in materialized {
        eprintln!(
            "warning: {name} is a materialized view, which the schema model can't represent -- skipping it"
        );
    }

    Ok(rows
        .into_iter()
        .filter_map(|(name, sql)| match sql {
            Some(sql) => Some(View::new(
                None,
                name.as_str(),
                sql.trim(),
                Some(DatabaseType::Postgresql),
            )),
            None => {
                eprintln!(
                    "warning: definition of view {name} is not visible to this role -- skipping it"
                );
                None
            }
        })
        .collect())
}
