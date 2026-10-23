//! SQL Server introspection (via `tiberius`), the counterpart of the `postgres` module. The rows
//! it collects are assembled into a `DatabaseModel` by the same code as the Postgres reader.
//!
//! Every numeric catalog column is `CAST` to `int` in SQL, because tiberius does not coerce
//! between integer widths when reading a row.

mod columns;
mod constraints;
mod keys;
mod relations;
mod routines;
mod tables;
mod triggers;
mod views;

use crate::error::SchemaReverseEngineerError;
use crate::reader::{SchemaParts, assemble_schema};
use schema_model::model::database_model::DatabaseModel;
use tiberius::{Client, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub type SqlServerClient = Client<Compat<TcpStream>>;

/// Opens a connection from an ADO-style connection string
/// (`Server=tcp:host,1433;Database=db;User Id=sa;Password=...`). TLS is required unless the
/// string says otherwise with `Encrypt=`.
pub async fn connect(
    connection_string: &str,
    trust_server_certificate: bool,
) -> Result<SqlServerClient, SchemaReverseEngineerError> {
    let mut config = tiberius::Config::from_ado_string(connection_string).map_err(|e| {
        SchemaReverseEngineerError::Connection(format!("Invalid SQL Server connection string: {e}"))
    })?;
    // `from_ado_string` maps an absent `Encrypt` key to "login only"; keep TLS on by default.
    if !specifies_encrypt(connection_string) {
        config.encryption(tiberius::EncryptionLevel::Required);
    }
    if trust_server_certificate {
        config.trust_cert();
    }
    let tcp = TcpStream::connect(config.get_addr())
        .await
        .map_err(|e| SchemaReverseEngineerError::Connection(format!("Failed to connect: {e}")))?;
    tcp.set_nodelay(true)
        .map_err(|e| SchemaReverseEngineerError::Connection(e.to_string()))?;
    Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| SchemaReverseEngineerError::Connection(format!("Failed to authenticate: {e}")))
}

fn specifies_encrypt(connection_string: &str) -> bool {
    connection_string.split(';').any(|part| {
        part.split('=')
            .next()
            .is_some_and(|key| key.trim().eq_ignore_ascii_case("encrypt"))
    })
}

/// Introspects the given SQL Server schema (`db_schema`, e.g. `"dbo"`) of the connected
/// database and builds a `DatabaseModel` describing its tables, columns, keys, foreign keys,
/// check constraints, views, functions, procedures, and triggers (as raw `other_sql`).
pub async fn read_sqlserver_schema(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<DatabaseModel, SchemaReverseEngineerError> {
    let table_names = tables::list_tables(client, db_schema).await?;
    let all_columns = columns::list_columns(client, db_schema).await?;
    let table_keys = keys::list_keys(client, db_schema).await?;
    let table_relations = relations::list_foreign_keys(client, db_schema).await?;
    let table_constraints = constraints::list_constraints(client, db_schema).await?;
    let table_views = views::list_views(client, db_schema).await?;
    let (functions, procedures) = routines::list_routines(client, db_schema).await?;
    let other_sql = triggers::list_triggers(client, db_schema).await?;

    Ok(assemble_schema(
        db_schema,
        SchemaParts {
            table_names,
            enum_types: Vec::new(),
            columns: all_columns,
            keys: table_keys,
            relations: table_relations,
            constraints: table_constraints,
            views: table_views,
            functions,
            procedures,
            other_sql,
        },
    ))
}

/// Runs `sql` with `db_schema` bound as `@P1` and returns the first result set.
async fn query(
    client: &mut SqlServerClient,
    sql: &str,
    db_schema: &str,
) -> Result<Vec<Row>, SchemaReverseEngineerError> {
    client
        .query(sql, &[&db_schema])
        .await
        .map_err(introspection_error)?
        .into_first_result()
        .await
        .map_err(introspection_error)
}

fn introspection_error(e: tiberius::error::Error) -> SchemaReverseEngineerError {
    SchemaReverseEngineerError::Introspection(e.to_string())
}

fn string(row: &Row, column: &str) -> Result<String, SchemaReverseEngineerError> {
    opt_string(row, column)?.ok_or_else(|| {
        SchemaReverseEngineerError::Introspection(format!("unexpected NULL in column {column}"))
    })
}

fn opt_string(row: &Row, column: &str) -> Result<Option<String>, SchemaReverseEngineerError> {
    row.try_get::<&str, _>(column)
        .map(|v| v.map(str::to_string))
        .map_err(introspection_error)
}

fn int(row: &Row, column: &str) -> Result<i32, SchemaReverseEngineerError> {
    row.try_get::<i32, _>(column)
        .map_err(introspection_error)?
        .ok_or_else(|| {
            SchemaReverseEngineerError::Introspection(format!("unexpected NULL in column {column}"))
        })
}

/// Strips every pair of parentheses that wraps the whole expression. SQL Server stores column
/// defaults and computed-column / check expressions wrapped (`((0))`, `(getdate())`,
/// `([a]*[b])`); the generators add their own punctuation, so the bare expression is kept.
pub(crate) fn strip_outer_parens(expression: &str) -> &str {
    let mut current = expression.trim();
    while current.starts_with('(') && current.ends_with(')') && wraps_whole(current) {
        current = current[1..current.len() - 1].trim();
    }
    current
}

/// True when the `(` at the start matches the `)` at the very end (so `(a)+(b)` is false).
fn wraps_whole(expression: &str) -> bool {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut chars = expression.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        match ch {
            '\'' if in_string => {
                if matches!(chars.peek(), Some((_, '\''))) {
                    chars.next();
                } else {
                    in_string = false;
                }
            }
            '\'' => in_string = true,
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                depth -= 1;
                if depth == 0 && i != expression.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_all_wrapping_layers() {
        assert_eq!(strip_outer_parens("((0))"), "0");
        assert_eq!(strip_outer_parens("(getdate())"), "getdate()");
        assert_eq!(strip_outer_parens("('A')"), "'A'");
    }

    #[test]
    fn keeps_parens_that_do_not_wrap_the_whole_expression() {
        assert_eq!(strip_outer_parens("(a)+(b)"), "(a)+(b)");
        assert_eq!(strip_outer_parens("((a)+(b))"), "(a)+(b)");
    }

    #[test]
    fn parens_inside_string_literals_are_ignored() {
        assert_eq!(strip_outer_parens("(')(')"), "')('");
        assert_eq!(strip_outer_parens("('it''s (x)')"), "'it''s (x)'");
    }

    #[test]
    fn detects_explicit_encrypt_key() {
        assert!(specifies_encrypt("Server=x;Encrypt=false;User Id=sa"));
        assert!(!specifies_encrypt("Server=x;TrustServerCertificate=true"));
    }
}
