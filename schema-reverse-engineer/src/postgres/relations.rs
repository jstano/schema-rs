use crate::error::SchemaReverseEngineerError;
use schema_model::model::relation::Relation;
use schema_model::model::types::RelationType;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, sqlx::FromRow)]
struct ForeignKeyRow {
    from_table: String,
    from_column: String,
    to_table: String,
    to_column: String,
    delete_rule: String,
}

/// Reads foreign keys for every table in `db_schema`, grouped by the owning (FK/child) table
/// name. Composite foreign keys are paired column-by-column using
/// `key_column_usage.position_in_unique_constraint`, which correctly lines up each FK column
/// with its referenced column regardless of declaration order (the Java original grouped by
/// `KEY_SEQ` per side independently, which risked mis-pairing columns for composite keys).
///
/// Uses `delete_rule` (not `update_rule`, which the Java original used) to determine cascade
/// behavior, since `ON DELETE` is what `RelationType` models.
pub async fn list_foreign_keys(
    pool: &PgPool,
    db_schema: &str,
) -> Result<HashMap<String, Vec<Relation>>, SchemaReverseEngineerError> {
    let rows: Vec<ForeignKeyRow> = sqlx::query_as(
        "SELECT tc.table_name AS from_table, fkcu.column_name AS from_column, \
                ucu.table_name AS to_table, ucu.column_name AS to_column, \
                rc.delete_rule AS delete_rule \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.referential_constraints rc \
           ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema \
         JOIN information_schema.key_column_usage fkcu \
           ON fkcu.constraint_name = tc.constraint_name AND fkcu.table_schema = tc.table_schema \
         JOIN information_schema.key_column_usage ucu \
           ON ucu.constraint_name = rc.unique_constraint_name \
          AND ucu.constraint_schema = rc.unique_constraint_schema \
          AND ucu.ordinal_position = fkcu.position_in_unique_constraint \
         WHERE tc.table_schema = $1 AND tc.constraint_type = 'FOREIGN KEY' \
         ORDER BY tc.constraint_name, fkcu.ordinal_position",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    let mut by_table: HashMap<String, Vec<Relation>> = HashMap::new();
    for row in rows {
        let relation_type = map_delete_rule(&row.delete_rule);
        let relation = Relation::new(
            row.to_table,
            row.to_column,
            row.from_table.clone(),
            row.from_column,
            relation_type,
            false,
        );
        by_table.entry(row.from_table).or_default().push(relation);
    }

    Ok(by_table)
}

fn map_delete_rule(delete_rule: &str) -> RelationType {
    match delete_rule {
        "CASCADE" => RelationType::Cascade,
        "SET NULL" | "SET DEFAULT" => RelationType::SetNull,
        "RESTRICT" | "NO ACTION" => RelationType::Enforce,
        _ => RelationType::DoNothing,
    }
}
