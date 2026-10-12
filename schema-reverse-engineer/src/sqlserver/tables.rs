use super::{SqlServerClient, query, string};
use crate::error::SchemaReverseEngineerError;

pub async fn list_tables(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<Vec<String>, SchemaReverseEngineerError> {
    let rows = query(
        client,
        "SELECT t.name AS table_name \
         FROM sys.tables t \
         WHERE t.schema_id = SCHEMA_ID(@P1) AND t.is_ms_shipped = 0 \
         ORDER BY t.name",
        db_schema,
    )
    .await?;
    rows.iter().map(|r| string(r, "table_name")).collect()
}
