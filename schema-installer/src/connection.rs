use crate::error::SchemaInstallerError;
use crate::migration::{AppliedMigration, AppliedRepeatableMigration, MigrationStatus};
use crate::tracking::SchemaMigrationDdl;
use schema_sql_generator::common::generator_type::GeneratorType;
use sqlx::{
    Pool, Postgres, Sqlite,
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::str::FromStr;
use std::time::Instant;
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

/// Whether an ADO.NET-style connection string contains an `Encrypt` key (case-insensitive).
fn ado_specifies_encrypt(connection_string: &str) -> bool {
    connection_string.split(';').any(|pair| {
        pair.split_once('=')
            .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("encrypt"))
    })
}

/// Which tracking table a shared bookkeeping operation targets. Only the operations whose SQL
/// differs by table name alone go through this; reads and inserts, whose columns differ, have
/// dedicated `*_repeatable*` methods instead.
#[derive(Clone, Copy)]
enum HistoryTable {
    Migration,
    RepeatableMigration,
}

impl HistoryTable {
    /// A fixed string from this enum - never user input - so it is safe to splice into SQL.
    fn name(self) -> &'static str {
        match self {
            HistoryTable::Migration => "schema_migration",
            HistoryTable::RepeatableMigration => "schema_repeatable_migration",
        }
    }
}

pub enum AnyPool {
    Postgresql(Pool<Postgres>),
    Sqlite(Pool<Sqlite>),
    SqlServer(Box<tokio::sync::Mutex<Client<Compat<TcpStream>>>>),
}

/// Row shape of `schema_migration` as read from PostgreSQL (`installed_at` is a native timestamp).
type PostgresMigrationRow = (
    i64,
    String,
    String,
    String,
    String,
    i32,
    chrono::DateTime<chrono::Utc>,
    String,
    String,
);
/// Row shape of `schema_migration` as read from SQLite (`installed_at` is text).
type SqliteMigrationRow = (
    i64,
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    String,
);

/// Row shapes of `schema_repeatable_migration`, as for the types above.
type PostgresRepeatableRow = (
    i64,
    String,
    String,
    String,
    i32,
    chrono::DateTime<chrono::Utc>,
    String,
    String,
);
type SqliteRepeatableRow = (i64, String, String, String, i64, String, String, String);

impl AnyPool {
    pub async fn connect(
        database_type: &GeneratorType,
        connection_string: &str,
        trust_server_certificate: bool,
    ) -> Result<Self, SchemaInstallerError> {
        match database_type {
            GeneratorType::Postgresql => {
                let pool = PgPoolOptions::new()
                    .max_connections(5)
                    .connect(connection_string)
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?;
                Ok(AnyPool::Postgresql(pool))
            }
            GeneratorType::Sqlite => {
                // Without a busy timeout, a second process hitting the same file while
                // this one holds a write lock (e.g., two instances racing to migrate)
                // fails immediately with "database is locked" instead of waiting its
                // turn, so it never gets a chance to observe the winner's committed
                // migration and back off cleanly.
                let options = SqliteConnectOptions::from_str(connection_string)
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?
                    .create_if_missing(true)
                    .busy_timeout(std::time::Duration::from_secs(10));

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(options)
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?;
                Ok(AnyPool::Sqlite(pool))
            }
            GeneratorType::SqlServer => {
                let mut config =
                    tiberius::Config::from_ado_string(connection_string).map_err(|e| {
                        SchemaInstallerError::Connection(format!(
                            "Invalid SQL Server connection string: {}",
                            e
                        ))
                    })?;

                // `from_ado_string` honours an explicit `Encrypt=`, but maps an *absent* key to
                // `Off` (login-only encryption). Keep TLS required by default and only defer
                // to the driver's parse when the user actually spelled the setting out.
                if !ado_specifies_encrypt(connection_string) {
                    config.encryption(tiberius::EncryptionLevel::Required);
                }
                if trust_server_certificate {
                    config.trust_cert();
                }

                let tcp = TcpStream::connect(config.get_addr()).await.map_err(|e| {
                    SchemaInstallerError::Connection(format!(
                        "Failed to connect to SQL Server: {}",
                        e
                    ))
                })?;

                tcp.set_nodelay(true).map_err(|e| {
                    SchemaInstallerError::Connection(format!("Failed to set TCP_NODELAY: {}", e))
                })?;

                let client = Client::connect(config, tcp.compat()).await.map_err(|e| {
                    SchemaInstallerError::Connection(format!(
                        "Failed to authenticate with SQL Server: {}",
                        e
                    ))
                })?;

                Ok(AnyPool::SqlServer(Box::new(tokio::sync::Mutex::new(
                    client,
                ))))
            }
        }
    }

    pub async fn execute_sql(&self, sql: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query(sql)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query(sql)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(sql, &[])
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn ensure_migration_table(
        &self,
        database_type: &GeneratorType,
    ) -> Result<(), SchemaInstallerError> {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(database_type);
        self.execute_sql(&ddl).await?;

        // `schema_migration_ddl`'s `CREATE TABLE IF NOT EXISTS` guard never touches a table
        // that already exists, so a SQL Server database that had the table created before the
        // NVARCHAR(MAX) -> bounded-length fix would otherwise fail with error 1919 forever.
        // Run the idempotent repair unconditionally here (it no-ops against an already-correct
        // table) instead of requiring the operator to run manual ALTER TABLE statements.
        if matches!(database_type, GeneratorType::SqlServer) {
            self.execute_sql(&SchemaMigrationDdl::sqlserver_repair_ddl())
                .await?;
        }

        // Same story for the `description` column added after the table first shipped (M14).
        if let Some(ddl) = SchemaMigrationDdl::add_description_column_ddl(database_type) {
            self.execute_sql(&ddl).await?;
        } else if let AnyPool::Sqlite(pool) = self {
            let (count,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM pragma_table_info('schema_migration') WHERE name = 'description'",
            )
            .fetch_one(pool)
            .await
            .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
            if count == 0 {
                self.execute_sql(SchemaMigrationDdl::SQLITE_ADD_DESCRIPTION_COLUMN_DDL)
                    .await?;
            }
        }

        // Repeatable migrations are tracked in their own table (see
        // `SchemaMigrationDdl::repeatable_migration_ddl`).
        self.execute_sql(&SchemaMigrationDdl::repeatable_migration_ddl(database_type))
            .await?;

        Ok(())
    }

    /// Executes a migration's already-split SQL statements and marks its tracking row
    /// `"success"` as **one** database transaction, returning the elapsed milliseconds on
    /// success.
    ///
    /// Previously the DDL and the status update were two separate transactions (insert
    /// "pending", commit the DDL, then a third autocommit `UPDATE ... status = 'success'`),
    /// which left a real gap: a process killed after the DDL commit but before the status
    /// update left the migration permanently applied while its tracking row stayed
    /// "pending" forever - wedging every future `migrate` behind the lock-wait timeout,
    /// with `repair`'s stale-pending cleanup then deleting the row and causing the next
    /// `migrate` to re-run DDL that already succeeded (BUGS_AND_GAPS H9). Folding the
    /// status update into the same transaction as the DDL means the two now commit
    /// together or not at all: a crash before commit leaves the row exactly "pending" (safe
    /// to reclaim, since none of the DDL took effect either), and a crash after commit
    /// leaves it "success", always matching whether the DDL actually ran.
    ///
    /// On a statement failure the transaction is rolled back (by never committing it, or
    /// explicitly for SQL Server) and this returns `Err` without touching the tracking row
    /// at all - the caller then records `"failed"` as a separate, deliberately
    /// non-atomic `update_migration_status` call, since a failed attempt has no DDL
    /// side effect it needs to stay atomic with.
    async fn execute_transactional_in(
        &self,
        table: HistoryTable,
        statements: &[String],
        migration_id: i64,
    ) -> Result<i64, SchemaInstallerError> {
        let start = Instant::now();
        match self {
            AnyPool::Postgresql(pool) => {
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                for statement in statements {
                    sqlx::query(statement.as_str())
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                }
                let elapsed_ms = start.elapsed().as_millis() as i64;
                let rows_affected = sqlx::query(&format!(
                    "UPDATE {} SET status = 'success', execution_time_ms = $1 WHERE id = $2",
                    table.name()
                ))
                .bind(elapsed_ms)
                .bind(migration_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?
                .rows_affected();
                if rows_affected == 0 {
                    // `tx` drops here without a `commit()`, rolling back the DDL along
                    // with it - see the "no tracking row found" comment on
                    // `update_migration_status` for why a silent no-op here would be worse
                    // than failing loudly.
                    return Err(SchemaInstallerError::Database(format!(
                        "failed to record migration status as 'success': no tracking row found for id {} (it may have been removed by a concurrent `repair` run)",
                        migration_id
                    )));
                }
                tx.commit()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(elapsed_ms)
            }
            AnyPool::Sqlite(pool) => {
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                for statement in statements {
                    sqlx::query(statement.as_str())
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                }
                let elapsed_ms = start.elapsed().as_millis() as i64;
                let rows_affected = sqlx::query(&format!(
                    "UPDATE {} SET status = 'success', execution_time_ms = ? WHERE id = ?",
                    table.name()
                ))
                .bind(elapsed_ms)
                .bind(migration_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?
                .rows_affected();
                if rows_affected == 0 {
                    return Err(SchemaInstallerError::Database(format!(
                        "failed to record migration status as 'success': no tracking row found for id {} (it may have been removed by a concurrent `repair` run)",
                        migration_id
                    )));
                }
                tx.commit()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(elapsed_ms)
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                // `Client::execute`/`query` send the SQL via the `sp_executesql` RPC, which
                // SQL Server treats as a nested procedure call: it checks that @@TRANCOUNT on
                // entry matches @@TRANCOUNT on exit, and raises error 266 ("mismatching number
                // of BEGIN and COMMIT statements") the instant a statement changes it - which
                // `BEGIN TRANSACTION` always does (0 -> 1). `simple_query` sends a plain SQL
                // batch (the same wire format SSMS/sqlcmd use) with no such check, so it's the
                // only way to issue transaction control statements here.
                client
                    .simple_query("BEGIN TRANSACTION")
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?
                    .into_results()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;

                for statement in statements {
                    let result = match client.simple_query(statement.as_str()).await {
                        Ok(stream) => stream.into_results().await.map(|_| ()),
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        // Best-effort rollback; report the original statement error either way.
                        let _ = client.simple_query("ROLLBACK TRANSACTION").await;
                        return Err(SchemaInstallerError::Execution(e.to_string()));
                    }
                }

                let elapsed_ms = start.elapsed().as_millis() as i64;
                let rows_affected: u64 = match client
                    .execute(
                        format!("UPDATE {} SET status = 'success', execution_time_ms = @P1 WHERE id = @P2", table.name()),
                        &[&(elapsed_ms as i32), &migration_id],
                    )
                    .await
                {
                    Ok(result) => result.rows_affected().iter().sum(),
                    Err(e) => {
                        let _ = client.simple_query("ROLLBACK TRANSACTION").await;
                        return Err(SchemaInstallerError::Execution(e.to_string()));
                    }
                };
                if rows_affected == 0 {
                    let _ = client.simple_query("ROLLBACK TRANSACTION").await;
                    return Err(SchemaInstallerError::Database(format!(
                        "failed to record migration status as 'success': no tracking row found for id {} (it may have been removed by a concurrent `repair` run)",
                        migration_id
                    )));
                }

                client
                    .simple_query("COMMIT TRANSACTION")
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?
                    .into_results()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(elapsed_ms)
            }
        }
    }

    /// Executes a migration's already-split statements one at a time in autocommit mode -
    /// no wrapping transaction - on a single connection (so session state like `SET` carries
    /// across statements), returning the elapsed milliseconds (M18). Needed for statements
    /// that refuse to run inside a transaction (`CREATE INDEX CONCURRENTLY`, `VACUUM`,
    /// `ALTER TYPE ... ADD VALUE`), and the only mode in which a script may manage its own
    /// `BEGIN`/`COMMIT`.
    ///
    /// Unlike `execute_migration_transactional` this cannot be atomic with the tracking
    /// row: statements commit as they run, so a failure part-way leaves the earlier
    /// statements applied. The caller records `"failed"` and the operator must fix the
    /// leftovers by hand before `repair` and a retry.
    pub async fn execute_migration_non_transactional(
        &self,
        statements: &[String],
    ) -> Result<i64, SchemaInstallerError> {
        let start = Instant::now();
        match self {
            AnyPool::Postgresql(pool) => {
                let mut conn = pool
                    .acquire()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                for statement in statements {
                    sqlx::query(statement.as_str())
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                }
            }
            AnyPool::Sqlite(pool) => {
                let mut conn = pool
                    .acquire()
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                for statement in statements {
                    sqlx::query(statement.as_str())
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                }
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                // `simple_query`, not `execute`: see the note in `execute_migration_transactional`.
                for statement in statements {
                    client
                        .simple_query(statement.as_str())
                        .await
                        .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?
                        .into_results()
                        .await
                        .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                }
            }
        }
        Ok(start.elapsed().as_millis() as i64)
    }

    pub async fn get_applied_migrations(
        &self,
    ) -> Result<Vec<AppliedMigration>, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let rows: Vec<PostgresMigrationRow> =
                    sqlx::query_as(
                        "SELECT id, version, script_path, description, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY id"
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            id,
                            version,
                            script_path,
                            description,
                            checksum,
                            execution_time_ms,
                            installed_at,
                            status,
                            tool_version,
                        )| {
                            AppliedMigration {
                                id,
                                version,
                                script_path,
                                description,
                                checksum,
                                execution_time_ms: execution_time_ms as i64,
                                installed_at: installed_at.to_rfc3339(),
                                status,
                                tool_version,
                            }
                        },
                    )
                    .collect())
            }
            AnyPool::Sqlite(pool) => {
                let rows: Vec<SqliteMigrationRow> =
                    sqlx::query_as(
                        "SELECT id, version, script_path, description, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY id"
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            id,
                            version,
                            script_path,
                            description,
                            checksum,
                            execution_time_ms,
                            installed_at,
                            status,
                            tool_version,
                        )| {
                            AppliedMigration {
                                id,
                                version,
                                script_path,
                                description,
                                checksum,
                                execution_time_ms,
                                installed_at,
                                status,
                                tool_version,
                            }
                        },
                    )
                    .collect())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                let results = client
                    .query(
                        "SELECT id, version, script_path, description, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY id",
                        &[],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let mut migrations = Vec::new();
                for row in results
                    .into_first_result()
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                {
                    let id: i64 = row
                        .get(0)
                        .ok_or_else(|| SchemaInstallerError::Database("Missing id".to_string()))?;
                    let version: &str = row.get(1).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing version".to_string())
                    })?;
                    let script_path: &str = row.get(2).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing script_path".to_string())
                    })?;
                    let description: &str = row.get(3).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing description".to_string())
                    })?;
                    let checksum: &str = row.get(4).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing checksum".to_string())
                    })?;
                    let execution_time_ms: i32 = row.get(5).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing execution_time_ms".to_string())
                    })?;
                    // `DATETIME2` round-trips through tiberius as a naive (timezone-less)
                    // `chrono::NaiveDateTime`, not `chrono::DateTime<Utc>` - that FromSql impl
                    // matches only `DateTimeOffset` (SQL Server's separate `DATETIMEOFFSET`
                    // type). Reading it as `DateTime<Utc>` panics via `Row::get`'s internal
                    // `.unwrap()` as soon as the table has a row. `installed_at`'s default is
                    // `SYSUTCDATETIME()` (see `tracking.rs`), so the naive value is already UTC.
                    let installed_at: chrono::NaiveDateTime = row.get(6).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing installed_at".to_string())
                    })?;
                    let installed_at = installed_at.and_utc();
                    let status: &str = row.get(7).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing status".to_string())
                    })?;
                    let tool_version: &str = row.get(8).ok_or_else(|| {
                        SchemaInstallerError::Database("Missing tool_version".to_string())
                    })?;

                    migrations.push(AppliedMigration {
                        id,
                        version: version.to_string(),
                        script_path: script_path.to_string(),
                        description: description.to_string(),
                        checksum: checksum.to_string(),
                        execution_time_ms: execution_time_ms as i64,
                        installed_at: installed_at.to_rfc3339(),
                        status: status.to_string(),
                        tool_version: tool_version.to_string(),
                    });
                }

                Ok(migrations)
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // one argument per schema_migration column
    pub async fn insert_migration(
        &self,
        version: &str,
        script_path: &str,
        description: &str,
        checksum: &str,
        execution_time_ms: i64,
        status: &str,
        tool_version: &str,
    ) -> Result<i64, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let row: (i64,) = sqlx::query_as(
                    "INSERT INTO schema_migration (version, script_path, description, checksum, execution_time_ms, status, tool_version) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id"
                )
                .bind(version)
                .bind(script_path)
                .bind(description)
                .bind(checksum)
                .bind(execution_time_ms)
                .bind(status)
                .bind(tool_version)
                .fetch_one(pool)
                .await
                .map_err(|e| map_insert_error(e, version))?;
                Ok(row.0)
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO schema_migration (version, script_path, description, checksum, execution_time_ms, status, tool_version) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(version)
                .bind(script_path)
                .bind(description)
                .bind(checksum)
                .bind(execution_time_ms)
                .bind(status)
                .bind(tool_version)
                .execute(pool)
                .await
                .map_err(|e| map_insert_error(e, version))?;

                let id: (i64,) = sqlx::query_as(
                    "SELECT id FROM schema_migration WHERE version = ? ORDER BY id DESC LIMIT 1",
                )
                .bind(version)
                .fetch_one(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(id.0)
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        "INSERT INTO schema_migration (version, script_path, description, checksum, execution_time_ms, status, tool_version) VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7)",
                        &[&version, &script_path, &description, &checksum, &(execution_time_ms as i32), &status, &tool_version],
                    )
                    .await
                    .map_err(|e| map_tiberius_insert_error(e, version))?;

                let result = client
                    .query(
                        "SELECT id FROM schema_migration WHERE version = @P1 ORDER BY id DESC",
                        &[&version],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let rows = result
                    .into_first_result()
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let id: i64 = rows
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        SchemaInstallerError::Database("No inserted row found".to_string())
                    })?
                    .get(0)
                    .ok_or_else(|| {
                        SchemaInstallerError::Database("Invalid id in inserted row".to_string())
                    })?;

                Ok(id)
            }
        }
    }

    async fn update_status_in(
        &self,
        table: HistoryTable,
        id: i64,
        status: &str,
        execution_time_ms: i64,
    ) -> Result<(), SchemaInstallerError> {
        // Every call site relies on this succeeding to record that a migration it just
        // ran to completion is now tracked - if the row silently isn't there any more
        // (e.g. `repair()`'s stale-pending cleanup raced a still-running migration and
        // deleted its row), a plain "0 rows affected" success would let a migration that
        // really did apply go unrecorded, so `migrate()` would treat it as still
        // pending and try to re-apply it next time. Surface that as a loud error instead.
        let rows_affected = match self {
            AnyPool::Postgresql(pool) => sqlx::query(&format!(
                "UPDATE {} SET status = $1, execution_time_ms = $2 WHERE id = $3",
                table.name()
            ))
            .bind(status)
            .bind(execution_time_ms)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
            .rows_affected(),
            AnyPool::Sqlite(pool) => sqlx::query(&format!(
                "UPDATE {} SET status = ?, execution_time_ms = ? WHERE id = ?",
                table.name()
            ))
            .bind(status)
            .bind(execution_time_ms)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
            .rows_affected(),
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        format!(
                            "UPDATE {} SET status = @P1, execution_time_ms = @P2 WHERE id = @P3",
                            table.name()
                        ),
                        &[&status, &(execution_time_ms as i32), &id],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                    .rows_affected()
                    .iter()
                    .sum()
            }
        };

        if rows_affected == 0 {
            return Err(SchemaInstallerError::Database(format!(
                "failed to record migration status as '{}': no tracking row found for id {} (it may have been removed by a concurrent `repair` run)",
                status, id
            )));
        }

        Ok(())
    }

    async fn delete_failed_in(&self, table: HistoryTable) -> Result<u64, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let rows_affected =
                    sqlx::query(&format!("DELETE FROM {} WHERE status = $1", table.name()))
                        .bind(MigrationStatus::Failed.as_str())
                        .execute(pool)
                        .await
                        .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                        .rows_affected();
                Ok(rows_affected)
            }
            AnyPool::Sqlite(pool) => {
                let rows_affected =
                    sqlx::query(&format!("DELETE FROM {} WHERE status = ?", table.name()))
                        .bind(MigrationStatus::Failed.as_str())
                        .execute(pool)
                        .await
                        .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                        .rows_affected();
                Ok(rows_affected)
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                let result = client
                    .execute(
                        format!("DELETE FROM {} WHERE status = @P1", table.name()),
                        &[&MigrationStatus::Failed.as_str()],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(result.total())
            }
        }
    }

    /// Deletes "pending" tracking rows older than `older_than_seconds`. A row stays
    /// "pending" only for as long as the process that inserted it is actively running
    /// the migration (see `wait_for_slot` in migrator.rs); one still "pending" well past
    /// that window means the owning process crashed or was killed mid-migration and
    /// never got to mark it "success"/"failed", which would otherwise wedge every future
    /// `migrate()` call behind the lock-wait timeout forever with no way to recover.
    async fn delete_stale_pending_in(
        &self,
        table: HistoryTable,
        older_than_seconds: i64,
    ) -> Result<u64, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let rows_affected = sqlx::query(&format!(
                    "DELETE FROM {} WHERE status = $1 AND installed_at < now() - ($2 || ' seconds')::interval",
                    table.name()
                ))
                .bind(MigrationStatus::Pending.as_str())
                .bind(older_than_seconds.to_string())
                .execute(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                .rows_affected();
                Ok(rows_affected)
            }
            AnyPool::Sqlite(pool) => {
                let rows_affected = sqlx::query(&format!(
                    "DELETE FROM {} WHERE status = ? AND installed_at < datetime('now', '-' || ? || ' seconds')",
                    table.name()
                ))
                .bind(MigrationStatus::Pending.as_str())
                .bind(older_than_seconds)
                .execute(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                .rows_affected();
                Ok(rows_affected)
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                let result = client
                    .execute(
                        format!("DELETE FROM {} WHERE status = @P1 AND installed_at < DATEADD(second, -@P2, SYSUTCDATETIME())", table.name()),
                        &[&MigrationStatus::Pending.as_str(), &(older_than_seconds as i32)],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(result.total())
            }
        }
    }

    /// Deletes a single version's tracking row if - and only if - it's `"failed"`.
    /// `install()` uses this to clear a leftover row from a previous failed attempt
    /// before re-inserting a fresh `"pending"` one (H14): unlike `migrate()`, `install()`
    /// doesn't go through `wait_for_slot`'s lock/wait dance, so without this a retry's
    /// `insert_migration` would collide with the leftover row's `UNIQUE (version)` and
    /// surface as a bogus `ConcurrentMigrationDetected` with no concurrency involved.
    /// Scoped to `"failed"` (not `"pending"`) so it never removes a row a genuinely
    /// concurrent attempt is still in the middle of.
    pub async fn delete_failed_migration_by_version(
        &self,
        version: &str,
    ) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE version = $1 AND status = $2")
                    .bind(version)
                    .bind(MigrationStatus::Failed.as_str())
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE version = ? AND status = ?")
                    .bind(version)
                    .bind(MigrationStatus::Failed.as_str())
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        "DELETE FROM schema_migration WHERE version = @P1 AND status = @P2",
                        &[&version, &MigrationStatus::Failed.as_str()],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    async fn update_checksum_in(
        &self,
        table: HistoryTable,
        id: i64,
        checksum: &str,
    ) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query(&format!(
                    "UPDATE {} SET checksum = $1 WHERE id = $2",
                    table.name()
                ))
                .bind(checksum)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query(&format!(
                    "UPDATE {} SET checksum = ? WHERE id = ?",
                    table.name()
                ))
                .bind(checksum)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        format!("UPDATE {} SET checksum = @P1 WHERE id = @P2", table.name()),
                        &[&checksum, &id],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }
}

/// Versioned (`schema_migration`) entry points over the shared `*_in` helpers, followed by the
/// repeatable (`schema_repeatable_migration`) equivalents.
impl AnyPool {
    pub async fn execute_migration_transactional(
        &self,
        statements: &[String],
        migration_id: i64,
    ) -> Result<i64, SchemaInstallerError> {
        self.execute_transactional_in(HistoryTable::Migration, statements, migration_id)
            .await
    }

    pub async fn update_migration_status(
        &self,
        id: i64,
        status: &str,
        execution_time_ms: i64,
    ) -> Result<(), SchemaInstallerError> {
        self.update_status_in(HistoryTable::Migration, id, status, execution_time_ms)
            .await
    }

    pub async fn delete_failed_migrations(&self) -> Result<u64, SchemaInstallerError> {
        self.delete_failed_in(HistoryTable::Migration).await
    }

    pub async fn delete_stale_pending_migrations(
        &self,
        older_than_seconds: i64,
    ) -> Result<u64, SchemaInstallerError> {
        self.delete_stale_pending_in(HistoryTable::Migration, older_than_seconds)
            .await
    }

    pub async fn update_migration_checksum(
        &self,
        id: i64,
        checksum: &str,
    ) -> Result<(), SchemaInstallerError> {
        self.update_checksum_in(HistoryTable::Migration, id, checksum)
            .await
    }

    /// `execute_migration_transactional` for a repeatable: the statements and the
    /// `schema_repeatable_migration` row's `"success"` update commit or roll back together.
    pub async fn execute_repeatable_transactional(
        &self,
        statements: &[String],
        repeatable_id: i64,
    ) -> Result<i64, SchemaInstallerError> {
        self.execute_transactional_in(HistoryTable::RepeatableMigration, statements, repeatable_id)
            .await
    }

    pub async fn update_repeatable_status(
        &self,
        id: i64,
        status: &str,
        execution_time_ms: i64,
    ) -> Result<(), SchemaInstallerError> {
        self.update_status_in(
            HistoryTable::RepeatableMigration,
            id,
            status,
            execution_time_ms,
        )
        .await
    }

    pub async fn delete_failed_repeatables(&self) -> Result<u64, SchemaInstallerError> {
        self.delete_failed_in(HistoryTable::RepeatableMigration)
            .await
    }

    pub async fn delete_stale_pending_repeatables(
        &self,
        older_than_seconds: i64,
    ) -> Result<u64, SchemaInstallerError> {
        self.delete_stale_pending_in(HistoryTable::RepeatableMigration, older_than_seconds)
            .await
    }

    pub async fn update_repeatable_checksum(
        &self,
        id: i64,
        checksum: &str,
    ) -> Result<(), SchemaInstallerError> {
        self.update_checksum_in(HistoryTable::RepeatableMigration, id, checksum)
            .await
    }

    pub async fn get_applied_repeatables(
        &self,
    ) -> Result<Vec<AppliedRepeatableMigration>, SchemaInstallerError> {
        const COLUMNS: &str = "SELECT id, name, script_path, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_repeatable_migration ORDER BY id";
        match self {
            AnyPool::Postgresql(pool) => {
                let rows: Vec<PostgresRepeatableRow> = sqlx::query_as(COLUMNS)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            id,
                            name,
                            script_path,
                            checksum,
                            execution_time_ms,
                            installed_at,
                            status,
                            tool_version,
                        )| AppliedRepeatableMigration {
                            id,
                            name,
                            script_path,
                            checksum,
                            execution_time_ms: execution_time_ms as i64,
                            installed_at: installed_at.to_rfc3339(),
                            status,
                            tool_version,
                        },
                    )
                    .collect())
            }
            AnyPool::Sqlite(pool) => {
                let rows: Vec<SqliteRepeatableRow> = sqlx::query_as(COLUMNS)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            id,
                            name,
                            script_path,
                            checksum,
                            execution_time_ms,
                            installed_at,
                            status,
                            tool_version,
                        )| AppliedRepeatableMigration {
                            id,
                            name,
                            script_path,
                            checksum,
                            execution_time_ms,
                            installed_at,
                            status,
                            tool_version,
                        },
                    )
                    .collect())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                let results = client
                    .query(COLUMNS, &[])
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let missing =
                    |column: &str| SchemaInstallerError::Database(format!("Missing {}", column));
                let mut repeatables = Vec::new();
                for row in results
                    .into_first_result()
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                {
                    let id: i64 = row.get(0).ok_or_else(|| missing("id"))?;
                    let name: &str = row.get(1).ok_or_else(|| missing("name"))?;
                    let script_path: &str = row.get(2).ok_or_else(|| missing("script_path"))?;
                    let checksum: &str = row.get(3).ok_or_else(|| missing("checksum"))?;
                    let execution_time_ms: i32 =
                        row.get(4).ok_or_else(|| missing("execution_time_ms"))?;
                    // Naive `DATETIME2`, already UTC - see `get_applied_migrations`.
                    let installed_at: chrono::NaiveDateTime =
                        row.get(5).ok_or_else(|| missing("installed_at"))?;
                    let status: &str = row.get(6).ok_or_else(|| missing("status"))?;
                    let tool_version: &str = row.get(7).ok_or_else(|| missing("tool_version"))?;

                    repeatables.push(AppliedRepeatableMigration {
                        id,
                        name: name.to_string(),
                        script_path: script_path.to_string(),
                        checksum: checksum.to_string(),
                        execution_time_ms: execution_time_ms as i64,
                        installed_at: installed_at.and_utc().to_rfc3339(),
                        status: status.to_string(),
                        tool_version: tool_version.to_string(),
                    });
                }
                Ok(repeatables)
            }
        }
    }

    /// Inserts a repeatable's tracking row (normally `"pending"`), returning its id. The
    /// `UNIQUE (name)` constraint is the mutual exclusion between concurrent instances, so a
    /// collision is reported as `ConcurrentMigrationDetected`, like `insert_migration`.
    pub async fn insert_repeatable(
        &self,
        name: &str,
        script_path: &str,
        checksum: &str,
        execution_time_ms: i64,
        status: &str,
        tool_version: &str,
    ) -> Result<i64, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let row: (i64,) = sqlx::query_as(
                    "INSERT INTO schema_repeatable_migration (name, script_path, checksum, execution_time_ms, status, tool_version) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
                )
                .bind(name)
                .bind(script_path)
                .bind(checksum)
                .bind(execution_time_ms)
                .bind(status)
                .bind(tool_version)
                .fetch_one(pool)
                .await
                .map_err(|e| map_insert_error(e, name))?;
                Ok(row.0)
            }
            AnyPool::Sqlite(pool) => {
                let result = sqlx::query(
                    "INSERT INTO schema_repeatable_migration (name, script_path, checksum, execution_time_ms, status, tool_version) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(name)
                .bind(script_path)
                .bind(checksum)
                .bind(execution_time_ms)
                .bind(status)
                .bind(tool_version)
                .execute(pool)
                .await
                .map_err(|e| map_insert_error(e, name))?;
                Ok(result.last_insert_rowid())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        "INSERT INTO schema_repeatable_migration (name, script_path, checksum, execution_time_ms, status, tool_version) VALUES (@P1, @P2, @P3, @P4, @P5, @P6)",
                        &[&name, &script_path, &checksum, &(execution_time_ms as i32), &status, &tool_version],
                    )
                    .await
                    .map_err(|e| map_tiberius_insert_error(e, name))?;

                let rows = client
                    .query(
                        "SELECT id FROM schema_repeatable_migration WHERE name = @P1",
                        &[&name],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                    .into_first_result()
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                rows.into_iter()
                    .next()
                    .and_then(|row| row.get::<i64, _>(0))
                    .ok_or_else(|| {
                        SchemaInstallerError::Database("No inserted row found".to_string())
                    })
            }
        }
    }

    /// Atomically claims a changed repeatable for re-execution: flips its `"success"` row back
    /// to `"pending"` with the new checksum, but only if the row still carries
    /// `old_checksum` and is still `"success"`. Returns `true` if this caller won the claim.
    /// `false` means another process got there first (or already finished the same change), so
    /// the caller must re-read the row rather than run the script. `installed_at` is reset so
    /// the stale-pending cleanup measures from the claim, not from the previous run.
    pub async fn repend_repeatable(
        &self,
        id: i64,
        old_checksum: &str,
        new_checksum: &str,
        script_path: &str,
        tool_version: &str,
    ) -> Result<bool, SchemaInstallerError> {
        let pending = MigrationStatus::Pending.as_str();
        let success = MigrationStatus::Success.as_str();
        let rows_affected: u64 = match self {
            AnyPool::Postgresql(pool) => sqlx::query(
                "UPDATE schema_repeatable_migration SET status = $1, checksum = $2, script_path = $3, tool_version = $4, execution_time_ms = 0, installed_at = now() WHERE id = $5 AND checksum = $6 AND status = $7",
            )
            .bind(pending)
            .bind(new_checksum)
            .bind(script_path)
            .bind(tool_version)
            .bind(id)
            .bind(old_checksum)
            .bind(success)
            .execute(pool)
            .await
            .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
            .rows_affected(),
            AnyPool::Sqlite(pool) => sqlx::query(
                "UPDATE schema_repeatable_migration SET status = ?, checksum = ?, script_path = ?, tool_version = ?, execution_time_ms = 0, installed_at = CURRENT_TIMESTAMP WHERE id = ? AND checksum = ? AND status = ?",
            )
            .bind(pending)
            .bind(new_checksum)
            .bind(script_path)
            .bind(tool_version)
            .bind(id)
            .bind(old_checksum)
            .bind(success)
            .execute(pool)
            .await
            .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
            .rows_affected(),
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        "UPDATE schema_repeatable_migration SET status = @P1, checksum = @P2, script_path = @P3, tool_version = @P4, execution_time_ms = 0, installed_at = SYSUTCDATETIME() WHERE id = @P5 AND checksum = @P6 AND status = @P7",
                        &[&pending, &new_checksum, &script_path, &tool_version, &id, &old_checksum, &success],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                    .total()
            }
        };
        Ok(rows_affected == 1)
    }
}

/// Maps a unique-constraint violation on `schema_migration.version` (Postgres or SQLite,
/// via sqlx) to a `ConcurrentMigrationDetected` error so a racing second instance gets a
/// clear, actionable error instead of a raw driver message. Any other error passes through
/// as a generic `Database` error.
fn map_insert_error(e: sqlx::Error, version: &str) -> SchemaInstallerError {
    if let sqlx::Error::Database(ref db_err) = e
        && db_err.kind() == sqlx::error::ErrorKind::UniqueViolation
    {
        return SchemaInstallerError::ConcurrentMigrationDetected(version.to_string());
    }
    SchemaInstallerError::Database(e.to_string())
}

/// Same mapping as `map_insert_error`, for the SQL Server (tiberius) driver, which reports
/// unique-constraint/unique-index violations as server error numbers 2627/2601.
fn map_tiberius_insert_error(e: tiberius::error::Error, version: &str) -> SchemaInstallerError {
    if let tiberius::error::Error::Server(token_error) = &e
        && matches!(token_error.code(), 2627 | 2601)
    {
        return SchemaInstallerError::ConcurrentMigrationDetected(version.to_string());
    }
    SchemaInstallerError::Database(e.to_string())
}

#[cfg(test)]
mod ado_encrypt_tests {
    use super::ado_specifies_encrypt;
    use tiberius::{Config, EncryptionLevel};

    #[test]
    fn detects_explicit_encrypt_key_case_insensitively() {
        assert!(ado_specifies_encrypt(
            "Server=tcp:h,1433;Encrypt=false;User=sa"
        ));
        assert!(ado_specifies_encrypt("server=h; ENCRYPT = true"));
        assert!(!ado_specifies_encrypt(
            "Server=tcp:h,1433;User=sa;Password=encrypt=x"
        ));
        assert!(!ado_specifies_encrypt("Server=tcp:h,1433;User=sa"));
    }

    #[test]
    fn driver_parse_honours_explicit_encrypt_false() {
        // Guards the premise of the fix: with no override applied, `Encrypt=false` survives.
        let cfg =
            Config::from_ado_string("Server=tcp:h,1433;User=sa;Password=p;Encrypt=false").unwrap();
        let dbg = format!("{:?}", cfg);
        assert!(dbg.contains("Off"), "{dbg}");
        let _ = EncryptionLevel::Off;
    }
}
