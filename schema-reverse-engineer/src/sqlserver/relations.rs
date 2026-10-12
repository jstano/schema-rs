use super::{SqlServerClient, query, string};
use crate::error::SchemaReverseEngineerError;
use crate::postgres::relations::{ForeignKeyRow, group_foreign_key_rows};
use schema_model::model::relation::Relation;
use std::collections::HashMap;

pub async fn list_foreign_keys(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<HashMap<String, Vec<Relation>>, SchemaReverseEngineerError> {
    // `constraint_column_id` orders the columns of a composite key and pairs each referencing
    // column with its referenced column.
    let rows = query(
        client,
        "SELECT fk.name AS constraint_name, pt.name AS from_table, pc.name AS from_column, \
                rs.name AS to_schema, rt.name AS to_table, rc.name AS to_column, \
                fk.delete_referential_action_desc AS delete_rule \
         FROM sys.foreign_keys fk \
         JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id \
         JOIN sys.tables pt ON pt.object_id = fk.parent_object_id \
         JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id \
         JOIN sys.tables rt ON rt.object_id = fk.referenced_object_id \
         JOIN sys.schemas rs ON rs.schema_id = rt.schema_id \
         JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id \
         WHERE pt.schema_id = SCHEMA_ID(@P1) \
         ORDER BY fk.name, fkc.constraint_column_id",
        db_schema,
    )
    .await?;

    let rows = rows
        .iter()
        .map(|r| {
            Ok(ForeignKeyRow {
                constraint_name: string(r, "constraint_name")?,
                from_table: string(r, "from_table")?,
                from_column: string(r, "from_column")?,
                to_schema: string(r, "to_schema")?,
                to_table: string(r, "to_table")?,
                to_column: string(r, "to_column")?,
                // `NO_ACTION` / `SET_NULL` / `SET_DEFAULT` -> the spelling the shared mapping uses.
                delete_rule: string(r, "delete_rule")?.replace('_', " "),
            })
        })
        .collect::<Result<Vec<_>, SchemaReverseEngineerError>>()?;

    group_foreign_key_rows(rows, db_schema)
}
