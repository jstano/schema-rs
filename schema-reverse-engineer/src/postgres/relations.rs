use crate::error::SchemaReverseEngineerError;
use schema_model::model::relation::Relation;
use schema_model::model::types::RelationType;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ForeignKeyRow {
    pub(crate) constraint_name: String,
    pub(crate) from_table: String,
    pub(crate) from_column: String,
    pub(crate) to_schema: String,
    pub(crate) to_table: String,
    pub(crate) to_column: String,
    pub(crate) delete_rule: String,
}

/// One foreign-key constraint's worth of grouped rows, in column-declaration order.
struct ConstraintGroup {
    from_table: String,
    to_schema: String,
    to_table: String,
    delete_rule: String,
    column_pairs: Vec<(String, String)>,
}

/// Reads foreign keys for every table in `db_schema`, grouped by the owning (FK/child) table
/// name. Composite foreign keys are paired column-by-column using
/// `key_column_usage.position_in_unique_constraint`, which correctly lines up each FK column
/// with its referenced column regardless of declaration order (the Java original grouped by
/// `KEY_SEQ` per side independently, which risked mis-pairing columns for composite keys).
/// Rows belonging to the same FK constraint (composite or not) are then grouped by
/// `constraint_name` into a single `Relation`, rather than emitting one `Relation` per row.
///
/// Uses `delete_rule` (not `update_rule`, which the Java original used) to determine cascade
/// behavior, since `ON DELETE` is what `RelationType` models.
pub async fn list_foreign_keys(
    pool: &PgPool,
    db_schema: &str,
) -> Result<HashMap<String, Vec<Relation>>, SchemaReverseEngineerError> {
    let rows: Vec<ForeignKeyRow> = sqlx::query_as(
        "SELECT tc.constraint_name AS constraint_name, \
                tc.table_name AS from_table, fkcu.column_name AS from_column, \
                ucu.table_schema AS to_schema, ucu.table_name AS to_table, ucu.column_name AS to_column, \
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

    group_foreign_key_rows(rows, db_schema)
}

/// Groups pre-sorted `(constraint_name, ordinal_position)` rows into one `Relation` per
/// FK constraint (single-column or composite), keyed by the owning (FK/child) table name.
/// Consecutive rows sharing a `constraint_name` are folded into one group; this relies on the
/// caller having ordered rows by `constraint_name, ordinal_position` (the SQL query does this).
pub(crate) fn group_foreign_key_rows(
    rows: Vec<ForeignKeyRow>,
    db_schema: &str,
) -> Result<HashMap<String, Vec<Relation>>, SchemaReverseEngineerError> {
    let mut groups: Vec<(String, ConstraintGroup)> = Vec::new();
    for row in rows {
        if let Some((last_name, group)) = groups.last_mut()
            && *last_name == row.constraint_name
        {
            group.column_pairs.push((row.from_column, row.to_column));
            continue;
        }
        groups.push((
            row.constraint_name,
            ConstraintGroup {
                from_table: row.from_table,
                to_schema: row.to_schema,
                to_table: row.to_table,
                delete_rule: row.delete_rule,
                column_pairs: vec![(row.from_column, row.to_column)],
            },
        ));
    }

    let mut by_table: HashMap<String, Vec<Relation>> = HashMap::new();
    for (constraint_name, group) in groups {
        // A target in another schema is written schema-qualified (`other.parent`), which the
        // parser resolves via `split_schema_table`. That schema isn't part of this run, so the
        // generated XML only parses once it is merged with that schema's own reverse-engineer
        // output.
        let to_table = if group.to_schema == db_schema {
            group.to_table
        } else {
            eprintln!(
                "warning: foreign key {}.{} references {}.{} in another schema -- also reverse-engineer that schema, or the generated XML won't parse",
                group.from_table, constraint_name, group.to_schema, group.to_table
            );
            format!("{}.{}", group.to_schema, group.to_table)
        };
        let relation_type = map_delete_rule(&group.delete_rule).map_err(|e| {
            SchemaReverseEngineerError::Introspection(format!(
                "foreign key constraint '{constraint_name}': {e}"
            ))
        })?;
        let relation = if group.column_pairs.len() == 1 {
            let (from_column, to_column) = group.column_pairs.into_iter().next().unwrap();
            Relation::new(
                to_table,
                to_column,
                group.from_table.clone(),
                from_column,
                relation_type,
                false,
            )
        } else {
            Relation::new_composite(
                to_table,
                group.from_table.clone(),
                group.column_pairs,
                relation_type,
                false,
            )
            .map_err(|e| {
                SchemaReverseEngineerError::Introspection(format!(
                    "foreign key constraint '{constraint_name}': {e}"
                ))
            })?
        };
        by_table.entry(group.from_table).or_default().push(relation);
    }

    Ok(by_table)
}

/// `SET DEFAULT` has no `RelationType` equivalent; mapping it to `SetNull` would null the
/// column instead of restoring its default, so it (and any unknown rule) is rejected.
fn map_delete_rule(delete_rule: &str) -> Result<RelationType, String> {
    match delete_rule {
        "CASCADE" => Ok(RelationType::Cascade),
        "SET NULL" => Ok(RelationType::SetNull),
        "RESTRICT" | "NO ACTION" => Ok(RelationType::Enforce),
        other => Err(format!(
            "unsupported ON DELETE rule '{other}' (supported: CASCADE, SET NULL, RESTRICT, NO ACTION)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        constraint_name: &str,
        from_table: &str,
        from_column: &str,
        to_table: &str,
        to_column: &str,
        delete_rule: &str,
    ) -> ForeignKeyRow {
        ForeignKeyRow {
            constraint_name: constraint_name.to_string(),
            from_table: from_table.to_string(),
            from_column: from_column.to_string(),
            to_schema: "public".to_string(),
            to_table: to_table.to_string(),
            to_column: to_column.to_string(),
            delete_rule: delete_rule.to_string(),
        }
    }

    #[test]
    fn cross_schema_foreign_key_gets_a_qualified_target() {
        let mut other = row("fk_x", "child", "x_id", "parent_x", "id", "CASCADE");
        other.to_schema = "other".to_string();
        let rows = vec![
            other,
            row("fk_a", "child", "a_id", "parent_a", "id", "CASCADE"),
        ];
        let by_table = group_foreign_key_rows(rows, "public").unwrap();
        let relations = &by_table["child"];
        assert_eq!(relations.len(), 2);
        assert_eq!(relations[0].to_table_name(), "other.parent_x");
        assert_eq!(relations[1].to_table_name(), "parent_a");
    }

    #[test]
    fn set_default_delete_rule_is_rejected() {
        let rows = vec![row(
            "fk_a",
            "child",
            "a_id",
            "parent_a",
            "id",
            "SET DEFAULT",
        )];
        let err = group_foreign_key_rows(rows, "public")
            .unwrap_err()
            .to_string();
        assert!(err.contains("SET DEFAULT"), "got: {err}");
    }

    #[test]
    fn single_column_rows_produce_one_relation_per_row() {
        let rows = vec![row("fk_a", "child", "parent_id", "parent", "id", "CASCADE")];
        let by_table = group_foreign_key_rows(rows, "public").unwrap();
        let relations = &by_table["child"];
        assert_eq!(relations.len(), 1);
        assert!(!relations[0].is_composite());
        assert_eq!(relations[0].from_column_name(), "parent_id");
        assert_eq!(relations[0].to_column_name(), "id");
        assert_eq!(relations[0].relation_type(), RelationType::Cascade);
    }

    #[test]
    fn multi_row_same_constraint_groups_into_one_composite_relation() {
        let rows = vec![
            row(
                "fk_assignment",
                "Assignment",
                "ParentAssignmentID",
                "Assignment",
                "ID",
                "CASCADE",
            ),
            row(
                "fk_assignment",
                "Assignment",
                "PropertyID",
                "Assignment",
                "PropertyID",
                "CASCADE",
            ),
        ];
        let by_table = group_foreign_key_rows(rows, "public").unwrap();
        let relations = &by_table["Assignment"];
        assert_eq!(relations.len(), 1);
        assert!(relations[0].is_composite());
        assert_eq!(
            relations[0].column_pairs(),
            &[
                ("ParentAssignmentID".to_string(), "ID".to_string()),
                ("PropertyID".to_string(), "PropertyID".to_string()),
            ]
        );
    }

    #[test]
    fn rows_from_different_constraints_stay_separate_even_when_adjacent() {
        let rows = vec![
            row("fk_a", "child", "a_id", "parent_a", "id", "CASCADE"),
            row("fk_b", "child", "b_id", "parent_b", "id", "SET NULL"),
        ];
        let by_table = group_foreign_key_rows(rows, "public").unwrap();
        let relations = &by_table["child"];
        assert_eq!(relations.len(), 2);
        assert_eq!(relations[0].to_table_name(), "parent_a");
        assert_eq!(relations[1].to_table_name(), "parent_b");
        assert_eq!(relations[1].relation_type(), RelationType::SetNull);
    }
}
