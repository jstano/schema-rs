use crate::error::SchemaReverseEngineerError;
use schema_model::model::function::Function;
use schema_model::model::procedure::Procedure;
use schema_model::model::types::DatabaseType;
use sqlx::PgPool;

/// The uuid generator that `schema-sql-generator` emits in its PostgreSQL header. Round-tripping
/// it would make the regenerated script define it twice, so it is left to the generator.
const GENERATED_UUID_FUNCTION: &str = "generate_uuid";

/// Plain functions and procedures of `db_schema`, as `create or replace ...` statements.
/// Aggregates and window functions are skipped (`pg_get_functiondef` rejects aggregates and the
/// model has no way to express them), as are functions owned by an extension.
pub async fn list_routines(
    pool: &PgPool,
    db_schema: &str,
) -> Result<(Vec<Function>, Vec<Procedure>), SchemaReverseEngineerError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT p.proname::text, p.prokind::text, pg_get_functiondef(p.oid) \
         FROM pg_proc p \
         JOIN pg_namespace n ON n.oid = p.pronamespace \
         WHERE n.nspname = $1 \
           AND p.prokind IN ('f', 'p') \
           AND NOT EXISTS ( \
               SELECT 1 FROM pg_depend d WHERE d.objid = p.oid AND d.deptype = 'e') \
         ORDER BY p.proname, p.oid",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    let mut functions = Vec::new();
    let mut procedures = Vec::new();
    for (name, kind, definition) in rows {
        let sql = definition
            .trim()
            .trim_end_matches(';')
            .trim_end()
            .to_string();
        if kind == "p" {
            procedures.push(Procedure::new(None, name, DatabaseType::Postgresql, sql));
        } else if name == GENERATED_UUID_FUNCTION {
            eprintln!(
                "note: function {name} is created by the PostgreSQL generator itself -- not exporting it"
            );
        } else {
            functions.push(Function::new(None, name, DatabaseType::Postgresql, sql));
        }
    }
    Ok((functions, procedures))
}
