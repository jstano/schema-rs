use crate::error::SchemaReverseEngineerError;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

pub use crate::info::{KeyInfo, TableKeys};

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
    /// `None` for an expression slot in the index (`attnum = 0`).
    column_name: Option<String>,
    is_include: bool,
    predicate: Option<String>,
}

/// Reads primary keys plus unique/non-unique indexes for every table in `db_schema` and
/// merges them into one `TableKeys` per table.
///
/// `exclusion_constraint_names` skips unique indexes that merely back a Postgres exclusion
/// constraint (already captured separately as a `Constraint`), matching the equivalent
/// `showIncludeKey` check in the original Java reverse-engineer tool.
///
/// Partial indexes keep their predicate (`filter`) and covering indexes their `INCLUDE`
/// columns. Expression indexes can't be modeled, so they are skipped with a warning.
/// Promoting a lone unique key to primary is left to the caller, which knows column nullability.
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
            .get_or_insert_with(KeyInfo::default)
            .columns
            .push(row.column_name);
    }

    let index_rows: Vec<IndexRow> = sqlx::query_as(
        "SELECT t.relname AS table_name, i.relname AS index_name, ix.indisunique AS is_unique, \
                ix.indisprimary AS is_primary, a.attname AS column_name, \
                (k.ord > ix.indnkeyatts) AS is_include, \
                pg_get_expr(ix.indpred, ix.indrelid) AS predicate \
         FROM pg_index ix \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         CROSS JOIN LATERAL unnest(ix.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
         LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum \
         WHERE n.nspname = $1 AND t.relkind IN ('r', 'p') \
         ORDER BY t.relname, i.relname, k.ord",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    apply_index_rows(&mut result, index_rows, exclusion_constraint_names);

    Ok(result)
}

/// Folds pre-sorted index rows into `result`.
///
/// Grouped by hand instead of via a `HashMap` keyed on `(table_name, index_name)`: the query
/// is already `ORDER BY t.relname, i.relname, ...`, so consecutive rows belong to the same
/// index -- iterating a `HashMap` here would silently discard that order, making two runs
/// against an unchanged database produce differently-ordered XML.
fn apply_index_rows(
    result: &mut HashMap<String, TableKeys>,
    index_rows: Vec<IndexRow>,
    exclusion_constraint_names: &HashSet<(String, String)>,
) {
    struct Group {
        table_name: String,
        index_name: String,
        is_primary: bool,
        has_expression: bool,
        info: KeyInfo,
    }
    let mut grouped: Vec<Group> = Vec::new();
    for row in index_rows {
        let is_new = !matches!(
            grouped.last(),
            Some(g) if g.table_name == row.table_name && g.index_name == row.index_name
        );
        if is_new {
            grouped.push(Group {
                table_name: row.table_name.clone(),
                index_name: row.index_name.clone(),
                is_primary: row.is_primary,
                has_expression: false,
                info: KeyInfo {
                    unique: row.is_unique,
                    filter: row.predicate.clone(),
                    ..KeyInfo::default()
                },
            });
        }
        let group = grouped.last_mut().unwrap();
        match row.column_name {
            None => group.has_expression = true,
            Some(name) if row.is_include => group.info.include.push(name),
            Some(name) => group.info.columns.push(name),
        }
    }

    for group in grouped {
        // The index backing the primary key is already represented via the PK query above.
        if group.is_primary {
            continue;
        }
        // Skip indexes that merely back an exclusion constraint (captured as a Constraint).
        if exclusion_constraint_names
            .contains(&(group.table_name.clone(), group.index_name.clone()))
        {
            continue;
        }
        if group.has_expression {
            eprintln!(
                "warning: index {}.{} is an expression index, which the schema model can't represent -- skipping it",
                group.table_name, group.index_name
            );
            continue;
        }

        let table_keys = result.entry(group.table_name).or_default();
        // `<unique>` can't carry `include`/`where`, so such an index is written as
        // `<index unique="true" ...>` instead.
        if group.info.unique && group.info.include.is_empty() && group.info.filter.is_none() {
            table_keys.unique.push(group.info);
        } else {
            table_keys.index.push(group.info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn row(
        index: &str,
        unique: bool,
        column: Option<&str>,
        include: bool,
        predicate: Option<&str>,
    ) -> IndexRow {
        IndexRow {
            table_name: "t".into(),
            index_name: index.into(),
            is_unique: unique,
            is_primary: false,
            column_name: column.map(String::from),
            is_include: include,
            predicate: predicate.map(String::from),
        }
    }

    fn apply(rows: Vec<IndexRow>) -> TableKeys {
        let mut result = HashMap::new();
        apply_index_rows(&mut result, rows, &HashSet::new());
        result.remove("t").unwrap_or_default()
    }

    #[test]
    fn plain_unique_index_is_a_unique_key() {
        let keys = apply(vec![row("u", true, Some("a"), false, None)]);
        assert_eq!(keys.unique.len(), 1);
        assert!(keys.index.is_empty());
    }

    #[test]
    fn partial_unique_index_keeps_filter_and_is_written_as_index() {
        let keys = apply(vec![row(
            "u",
            true,
            Some("a"),
            false,
            Some("(b IS NOT NULL)"),
        )]);
        assert!(keys.unique.is_empty());
        assert_eq!(keys.index.len(), 1);
        assert!(keys.index[0].unique);
        assert_eq!(keys.index[0].columns, vec!["a"]);
        assert_eq!(keys.index[0].filter.as_deref(), Some("(b IS NOT NULL)"));
    }

    #[test]
    fn include_columns_are_separated_from_key_columns() {
        let keys = apply(vec![
            row("i", false, Some("a"), false, None),
            row("i", false, Some("b"), true, None),
        ]);
        assert_eq!(keys.index[0].columns, vec!["a"]);
        assert_eq!(keys.index[0].include, vec!["b"]);
    }

    #[test]
    fn expression_index_is_skipped() {
        let keys = apply(vec![
            row("e", false, None, false, None),
            row("ok", false, Some("a"), false, None),
        ]);
        assert_eq!(keys.index.len(), 1);
        assert_eq!(keys.index[0].columns, vec!["a"]);
    }
}
