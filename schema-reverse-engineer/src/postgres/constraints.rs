use crate::error::SchemaReverseEngineerError;
use schema_model::model::constraint::Constraint;
use schema_model::model::types::DatabaseType;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

/// Check ('c') and exclusion ('x') constraints, grouped by owning table name.
/// Primary key ('p') and unique ('u') constraints are intentionally excluded here since
/// they are represented via table keys instead (see `keys.rs`).
pub async fn list_constraints(
    pool: &PgPool,
    db_schema: &str,
) -> Result<HashMap<String, Vec<Constraint>>, SchemaReverseEngineerError> {
    let rows = raw_constraint_rows(pool, db_schema).await?;

    let mut by_table: HashMap<String, Vec<Constraint>> = HashMap::new();
    for row in rows {
        by_table
            .entry(row.table_name)
            .or_default()
            .push(Constraint::new(row.name, row.definition, DatabaseType::Postgresql));
    }
    Ok(by_table)
}

/// Names of exclusion constraints ('x'), keyed by (table_name, constraint_name), so that
/// `keys.rs` can skip the unique index that merely backs an exclusion constraint.
pub async fn list_exclusion_constraint_names(
    pool: &PgPool,
    db_schema: &str,
) -> Result<HashSet<(String, String)>, SchemaReverseEngineerError> {
    let rows = raw_constraint_rows(pool, db_schema).await?;
    Ok(rows
        .into_iter()
        .filter(|r| r.contype == "x")
        .map(|r| (r.table_name, r.name))
        .collect())
}

#[derive(Debug, sqlx::FromRow)]
struct ConstraintRow {
    table_name: String,
    name: String,
    contype: String,
    definition: String,
}

async fn raw_constraint_rows(pool: &PgPool, db_schema: &str) -> Result<Vec<ConstraintRow>, SchemaReverseEngineerError> {
    sqlx::query_as::<_, ConstraintRow>(
        "SELECT c.relname AS table_name, con.conname AS name, con.contype::text AS contype, \
                pg_get_constraintdef(con.oid) AS definition \
         FROM pg_constraint con \
         JOIN pg_class c ON c.oid = con.conrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND con.contype IN ('x', 'c') \
         ORDER BY c.relname, con.conname",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))
}
