use super::{SqlServerClient, opt_string, query, string};
use crate::error::SchemaReverseEngineerError;
use schema_model::model::function::Function;
use schema_model::model::procedure::Procedure;
use schema_model::model::types::DatabaseType;

/// T-SQL functions (scalar `FN`, inline `IF`, table-valued `TF`) and stored procedures (`P`),
/// each as its full `CREATE ...` text from `sys.sql_modules`. CLR routines are skipped.
pub async fn list_routines(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<(Vec<Function>, Vec<Procedure>), SchemaReverseEngineerError> {
    let rows = query(
        client,
        "SELECT o.name AS name, RTRIM(o.type) AS kind, m.definition AS definition \
         FROM sys.objects o \
         JOIN sys.sql_modules m ON m.object_id = o.object_id \
         WHERE o.schema_id = SCHEMA_ID(@P1) AND o.is_ms_shipped = 0 \
           AND o.type IN ('FN', 'IF', 'TF', 'P') \
         ORDER BY o.name",
        db_schema,
    )
    .await?;

    let mut functions = Vec::new();
    let mut procedures = Vec::new();
    for row in &rows {
        let name = string(row, "name")?;
        let Some(definition) = opt_string(row, "definition")? else {
            eprintln!("warning: definition of {name} is not visible (encrypted?) -- skipping it");
            continue;
        };
        let sql = definition.trim().to_string();
        if string(row, "kind")? == "P" {
            procedures.push(Procedure::new(None, name, DatabaseType::SqlServer, sql));
        } else {
            functions.push(Function::new(None, name, DatabaseType::SqlServer, sql));
        }
    }
    Ok((functions, procedures))
}
