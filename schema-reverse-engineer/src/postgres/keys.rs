use crate::error::SchemaReverseEngineerError;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct KeyInfo {
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TableKeys {
    pub primary: Option<KeyInfo>,
    pub unique: Vec<KeyInfo>,
    pub index: Vec<KeyInfo>,
}

#[derive(Debug, sqlx::FromRow)]
struct PrimaryKeyRow {
    table_name: String,
    column_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct IndexRow {
    table_name: String,
    index_name: String,
    is_unique: bool,
    is_primary: bool,
    column_name: String,
}

/// Reads primary keys plus unique/non-unique indexes for every table in `db_schema` and
/// merges them into one `TableKeys` per table.
///
/// `exclusion_constraint_names` skips unique indexes that merely back a Postgres exclusion
/// constraint (already captured separately as a `Constraint`), matching the equivalent
/// `showIncludeKey` check in the original Java reverse-engineer tool.
///
/// If a table has no primary key but exactly one unique key, that unique key is promoted to
/// the primary key -- this mirrors a (surprising but intentional) heuristic in the Java
/// original: some tables only declare a single unique index rather than an explicit PK.
pub async fn list_keys(
    pool: &PgPool,
    db_schema: &str,
    exclusion_constraint_names: &HashSet<(String, String)>,
) -> Result<HashMap<String, TableKeys>, SchemaReverseEngineerError> {
    let mut result: HashMap<String, TableKeys> = HashMap::new();

    let pk_rows: Vec<PrimaryKeyRow> = sqlx::query_as(
        "SELECT tc.table_name, kcu.column_name \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
           ON kcu.constraint_name = tc.constraint_name AND kcu.table_schema = tc.table_schema \
         WHERE tc.table_schema = $1 AND tc.constraint_type = 'PRIMARY KEY' \
         ORDER BY tc.table_name, kcu.ordinal_position",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    for row in pk_rows {
        result
            .entry(row.table_name)
            .or_default()
            .primary
            .get_or_insert_with(|| KeyInfo { columns: Vec::new() })
            .columns
            .push(row.column_name);
    }

    let index_rows: Vec<IndexRow> = sqlx::query_as(
        "SELECT t.relname AS table_name, i.relname AS index_name, ix.indisunique AS is_unique, \
                ix.indisprimary AS is_primary, a.attname AS column_name \
         FROM pg_index ix \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey) \
         WHERE n.nspname = $1 AND t.relkind = 'r' \
         ORDER BY t.relname, i.relname, array_position(ix.indkey, a.attnum)",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    let mut by_index: HashMap<(String, String), (bool, bool, Vec<String>)> = HashMap::new();
    for row in index_rows {
        let entry = by_index
            .entry((row.table_name, row.index_name))
            .or_insert((row.is_unique, row.is_primary, Vec::new()));
        entry.2.push(row.column_name);
    }

    for ((table_name, index_name), (is_unique, is_primary, columns)) in by_index {
        // The index backing the primary key is already represented via the PK query above.
        if is_primary {
            continue;
        }
        // Skip indexes that merely back an exclusion constraint (captured as a Constraint).
        if exclusion_constraint_names.contains(&(table_name.clone(), index_name)) {
            continue;
        }

        let table_keys = result.entry(table_name).or_default();
        if is_unique {
            table_keys.unique.push(KeyInfo { columns });
        } else {
            table_keys.index.push(KeyInfo { columns });
        }
    }

    for table_keys in result.values_mut() {
        if table_keys.primary.is_none() && table_keys.unique.len() == 1 {
            let promoted = table_keys.unique.remove(0);
            table_keys.primary = Some(promoted);
        }
    }

    Ok(result)
}
