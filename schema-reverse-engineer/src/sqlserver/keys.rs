use super::{SqlServerClient, int, opt_string, query, string};
use crate::error::SchemaReverseEngineerError;
use crate::info::{KeyInfo, TableKeys};
use std::collections::HashMap;

/// `sys.indexes.type` values that are ordinary B-tree indexes.
const CLUSTERED: i32 = 1;
const NONCLUSTERED: i32 = 2;

#[derive(Debug)]
pub(crate) struct IndexRow {
    pub table_name: String,
    pub index_name: String,
    pub index_type: i32,
    pub type_desc: String,
    pub is_unique: bool,
    pub is_primary: bool,
    pub column_name: String,
    pub is_include: bool,
    pub filter: Option<String>,
}

pub async fn list_keys(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<HashMap<String, TableKeys>, SchemaReverseEngineerError> {
    let rows = query(
        client,
        "SELECT t.name AS table_name, i.name AS index_name, \
                CAST(i.type AS int) AS index_type, i.type_desc AS type_desc, \
                CAST(i.is_unique AS int) AS is_unique, \
                CAST(i.is_primary_key AS int) AS is_primary, \
                c.name AS column_name, \
                CAST(ic.is_included_column AS int) AS is_include, \
                i.filter_definition AS filter \
         FROM sys.indexes i \
         JOIN sys.tables t ON t.object_id = i.object_id \
         JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
         JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
         WHERE t.schema_id = SCHEMA_ID(@P1) AND t.is_ms_shipped = 0 \
           AND i.is_hypothetical = 0 AND i.type > 0 \
         ORDER BY t.name, i.index_id, ic.is_included_column, ic.key_ordinal, ic.index_column_id",
        db_schema,
    )
    .await?;

    let index_rows = rows
        .iter()
        .map(|r| {
            Ok(IndexRow {
                table_name: string(r, "table_name")?,
                index_name: string(r, "index_name")?,
                index_type: int(r, "index_type")?,
                type_desc: string(r, "type_desc")?,
                is_unique: int(r, "is_unique")? != 0,
                is_primary: int(r, "is_primary")? != 0,
                column_name: string(r, "column_name")?,
                is_include: int(r, "is_include")? != 0,
                filter: opt_string(r, "filter")?,
            })
        })
        .collect::<Result<Vec<_>, SchemaReverseEngineerError>>()?;

    Ok(apply_index_rows(index_rows))
}

/// Folds rows (already ordered by table, index, then key position) into one `TableKeys` per
/// table. Consecutive rows of the same index are grouped by hand rather than via a `HashMap`,
/// so the output order is stable across runs.
///
/// A primary key becomes `primary`; a unique index with no `INCLUDE` or filter becomes a
/// `unique` key (this also covers `UNIQUE` constraints); anything else is an `index`. Columnstore,
/// XML and spatial indexes can't be modeled and are skipped with a warning. Clustered-ness is
/// not carried over.
pub(crate) fn apply_index_rows(rows: Vec<IndexRow>) -> HashMap<String, TableKeys> {
    struct Group {
        table_name: String,
        index_name: String,
        index_type: i32,
        type_desc: String,
        is_primary: bool,
        info: KeyInfo,
    }

    let mut groups: Vec<Group> = Vec::new();
    for row in rows {
        let is_new = !matches!(
            groups.last(),
            Some(g) if g.table_name == row.table_name && g.index_name == row.index_name
        );
        if is_new {
            groups.push(Group {
                table_name: row.table_name.clone(),
                index_name: row.index_name.clone(),
                index_type: row.index_type,
                type_desc: row.type_desc.clone(),
                is_primary: row.is_primary,
                info: KeyInfo {
                    unique: row.is_unique,
                    filter: row.filter.clone(),
                    ..KeyInfo::default()
                },
            });
        }
        let group = groups.last_mut().unwrap();
        if row.is_include {
            group.info.include.push(row.column_name);
        } else {
            group.info.columns.push(row.column_name);
        }
    }

    let mut result: HashMap<String, TableKeys> = HashMap::new();
    for group in groups {
        if group.index_type != CLUSTERED && group.index_type != NONCLUSTERED {
            eprintln!(
                "warning: index {}.{} is a {} index, which the schema model can't represent -- skipping it",
                group.table_name,
                group.index_name,
                group.type_desc.to_lowercase().replace('_', " ")
            );
            continue;
        }
        let table_keys = result.entry(group.table_name).or_default();
        if group.is_primary {
            table_keys.primary = Some(group.info);
        } else if group.info.unique && group.info.include.is_empty() && group.info.filter.is_none()
        {
            table_keys.unique.push(group.info);
        } else {
            table_keys.index.push(group.info);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: &str, index_type: i32, unique: bool, primary: bool, column: &str) -> IndexRow {
        IndexRow {
            table_name: "t".into(),
            index_name: index.into(),
            index_type,
            type_desc: "NONCLUSTERED".into(),
            is_unique: unique,
            is_primary: primary,
            column_name: column.into(),
            is_include: false,
            filter: None,
        }
    }

    fn keys(rows: Vec<IndexRow>) -> TableKeys {
        apply_index_rows(rows).remove("t").unwrap_or_default()
    }

    #[test]
    fn composite_primary_key_keeps_column_order() {
        let k = keys(vec![
            row("pk", 1, true, true, "b"),
            row("pk", 1, true, true, "a"),
        ]);
        assert_eq!(k.primary.unwrap().columns, vec!["b", "a"]);
    }

    #[test]
    fn plain_unique_index_is_a_unique_key() {
        let k = keys(vec![row("u", 2, true, false, "a")]);
        assert_eq!(k.unique.len(), 1);
        assert!(k.index.is_empty());
    }

    #[test]
    fn filtered_unique_index_stays_an_index_with_its_filter() {
        let mut r = row("u", 2, true, false, "a");
        r.filter = Some("([b] IS NOT NULL)".into());
        let k = keys(vec![r]);
        assert!(k.unique.is_empty());
        assert!(k.index[0].unique);
        assert_eq!(k.index[0].filter.as_deref(), Some("([b] IS NOT NULL)"));
    }

    #[test]
    fn included_columns_are_separated_from_key_columns() {
        let mut inc = row("i", 2, false, false, "b");
        inc.is_include = true;
        let k = keys(vec![row("i", 2, false, false, "a"), inc]);
        assert_eq!(k.index[0].columns, vec!["a"]);
        assert_eq!(k.index[0].include, vec!["b"]);
    }

    #[test]
    fn unique_index_with_include_is_not_a_unique_key() {
        let mut inc = row("i", 2, true, false, "b");
        inc.is_include = true;
        let k = keys(vec![row("i", 2, true, false, "a"), inc]);
        assert!(k.unique.is_empty());
        assert_eq!(k.index.len(), 1);
    }

    #[test]
    fn columnstore_index_is_skipped() {
        let k = keys(vec![
            row("cs", 5, false, false, "a"),
            row("ok", 2, false, false, "a"),
        ]);
        assert_eq!(k.index.len(), 1);
    }
}
