use super::{SqlServerClient, opt_string, query, string};
use crate::error::SchemaReverseEngineerError;
use schema_model::model::other_sql::OtherSql;
use schema_model::model::types::{DatabaseType, OtherSqlOrder};

/// Table DML triggers as their full `CREATE TRIGGER` text from `sys.sql_modules`. The model's
/// typed `Trigger` is only a body snippet the generator wraps in its own header, which can't
/// represent a trigger written by hand (several events, its own name), so each one is exported
/// verbatim as bottom-ordered `other_sql`, after the tables it refers to. Database-level DDL
/// triggers and CLR triggers are skipped.
pub async fn list_triggers(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<Vec<OtherSql>, SchemaReverseEngineerError> {
    let rows = query(
        client,
        "SELECT tr.name AS name, m.definition AS definition \
         FROM sys.triggers tr \
         JOIN sys.tables t ON t.object_id = tr.parent_id \
         JOIN sys.sql_modules m ON m.object_id = tr.object_id \
         WHERE tr.parent_class = 1 AND tr.is_ms_shipped = 0 \
           AND t.schema_id = SCHEMA_ID(@P1) \
         ORDER BY t.name, tr.name",
        db_schema,
    )
    .await?;

    let mut triggers = Vec::new();
    for row in &rows {
        let name = string(row, "name")?;
        let Some(definition) = opt_string(row, "definition")? else {
            eprintln!(
                "warning: definition of trigger {name} is not visible (encrypted?) -- skipping it"
            );
            continue;
        };
        triggers.push(OtherSql::new(
            DatabaseType::SqlServer,
            OtherSqlOrder::Bottom,
            definition.trim(),
        ));
    }
    Ok(triggers)
}
