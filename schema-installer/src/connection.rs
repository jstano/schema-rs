use crate::error::SchemaInstallerError;
use crate::migration::AppliedMigration;
use crate::tracking::SchemaMigrationDdl;
use schema_sql_generator::common::generator_type::GeneratorType;
use sqlx::{Pool, Postgres, Sqlite, postgres::PgPoolOptions, sqlite::{SqliteConnectOptions, SqlitePoolOptions}};
use std::str::FromStr;
use std::time::Instant;
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

pub enum AnyPool {
    Postgresql(Pool<Postgres>),
    Sqlite(Pool<Sqlite>),
    SqlServer(tokio::sync::Mutex<Client<Compat<TcpStream>>>),
}

impl AnyPool {
    pub async fn connect(database_type: &GeneratorType, connection_string: &str) -> Result<Self, SchemaInstallerError> {
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
                let mut config = tiberius::Config::from_ado_string(connection_string)
                    .map_err(|e| SchemaInstallerError::Connection(format!("Invalid SQL Server connection string: {}", e)))?;

                config.encryption(tiberius::EncryptionLevel::Required);

                let tcp = TcpStream::connect(config.get_addr())
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(format!("Failed to connect to SQL Server: {}", e)))?;

                tcp.set_nodelay(true)
                    .map_err(|e| SchemaInstallerError::Connection(format!("Failed to set TCP_NODELAY: {}", e)))?;

                let client = Client::connect(config, tcp.compat())
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(format!("Failed to authenticate with SQL Server: {}", e)))?;

                Ok(AnyPool::SqlServer(tokio::sync::Mutex::new(client)))
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

    pub async fn ensure_migration_table(&self, database_type: &GeneratorType) -> Result<(), SchemaInstallerError> {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(database_type);
        self.execute_sql(&ddl).await?;

        // `schema_migration_ddl`'s `CREATE TABLE IF NOT EXISTS` guard never touches a table
        // that already exists, so a SQL Server database that had the table created before the
        // NVARCHAR(MAX) -> bounded-length fix would otherwise fail with error 1919 forever.
        // Run the idempotent repair unconditionally here (it no-ops against an already-correct
        // table) instead of requiring the operator to run manual ALTER TABLE statements.
        if matches!(database_type, GeneratorType::SqlServer) {
            self.execute_sql(&SchemaMigrationDdl::sqlserver_repair_ddl()).await?;
        }

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
    pub async fn execute_migration_transactional(
        &self,
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
                let rows_affected = sqlx::query("UPDATE schema_migration SET status = 'success', execution_time_ms = $1 WHERE id = $2")
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
                let rows_affected = sqlx::query("UPDATE schema_migration SET status = 'success', execution_time_ms = ? WHERE id = ?")
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
                client
                    .execute("BEGIN TRANSACTION", &[])
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;

                for statement in statements {
                    if let Err(e) = client.execute(statement.as_str(), &[]).await {
                        // Best-effort rollback; report the original statement error either way.
                        let _ = client.execute("ROLLBACK TRANSACTION", &[]).await;
                        return Err(SchemaInstallerError::Execution(e.to_string()));
                    }
                }

                let elapsed_ms = start.elapsed().as_millis() as i64;
                let rows_affected: u64 = match client
                    .execute(
                        "UPDATE schema_migration SET status = 'success', execution_time_ms = @P1 WHERE id = @P2",
                        &[&(elapsed_ms as i32), &migration_id],
                    )
                    .await
                {
                    Ok(result) => result.rows_affected().iter().sum(),
                    Err(e) => {
                        let _ = client.execute("ROLLBACK TRANSACTION", &[]).await;
                        return Err(SchemaInstallerError::Execution(e.to_string()));
                    }
                };
                if rows_affected == 0 {
                    let _ = client.execute("ROLLBACK TRANSACTION", &[]).await;
                    return Err(SchemaInstallerError::Database(format!(
                        "failed to record migration status as 'success': no tracking row found for id {} (it may have been removed by a concurrent `repair` run)",
                        migration_id
                    )));
                }

                client
                    .execute("COMMIT TRANSACTION", &[])
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(elapsed_ms)
            }
        }
    }

    pub async fn get_applied_migrations(&self) -> Result<Vec<AppliedMigration>, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let rows: Vec<(i64, String, String, String, i32, chrono::DateTime<chrono::Utc>, String, String)> =
                    sqlx::query_as(
                        "SELECT id, version, script_path, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY installed_at"
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                Ok(rows
                    .into_iter()
                    .map(
                        |(id, version, script_path, checksum, execution_time_ms, installed_at, status, tool_version)| {
                            AppliedMigration {
                                id,
                                version,
                                script_path,
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
                let rows: Vec<(i64, String, String, String, i64, String, String, String)> =
                    sqlx::query_as(
                        "SELECT id, version, script_path, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY installed_at"
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                Ok(rows
                    .into_iter()
                    .map(
                        |(id, version, script_path, checksum, execution_time_ms, installed_at, status, tool_version)| {
                            AppliedMigration {
                                id,
                                version,
                                script_path,
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
                        "SELECT id, version, script_path, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY installed_at",
                        &[],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let mut migrations = Vec::new();
                for row in results.into_first_result().await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))? {
                    let id: i64 = row.get(0).ok_or_else(|| SchemaInstallerError::Database("Missing id".to_string()))?;
                    let version: &str = row.get(1).ok_or_else(|| SchemaInstallerError::Database("Missing version".to_string()))?;
                    let script_path: &str = row.get(2).ok_or_else(|| SchemaInstallerError::Database("Missing script_path".to_string()))?;
                    let checksum: &str = row.get(3).ok_or_else(|| SchemaInstallerError::Database("Missing checksum".to_string()))?;
                    let execution_time_ms: i32 = row.get(4).ok_or_else(|| SchemaInstallerError::Database("Missing execution_time_ms".to_string()))?;
                    // `DATETIME2` round-trips through tiberius as a naive (timezone-less)
                    // `chrono::NaiveDateTime`, not `chrono::DateTime<Utc>` - that FromSql impl
                    // matches only `DateTimeOffset` (SQL Server's separate `DATETIMEOFFSET`
                    // type). Reading it as `DateTime<Utc>` panics via `Row::get`'s internal
                    // `.unwrap()` as soon as the table has a row. `installed_at`'s default is
                    // `SYSUTCDATETIME()` (see `tracking.rs`), so the naive value is already UTC.
                    let installed_at: chrono::NaiveDateTime = row.get(5).ok_or_else(|| SchemaInstallerError::Database("Missing installed_at".to_string()))?;
                    let installed_at = installed_at.and_utc();
                    let status: &str = row.get(6).ok_or_else(|| SchemaInstallerError::Database("Missing status".to_string()))?;
                    let tool_version: &str = row.get(7).ok_or_else(|| SchemaInstallerError::Database("Missing tool_version".to_string()))?;

                    migrations.push(AppliedMigration {
                        id,
                        version: version.to_string(),
                        script_path: script_path.to_string(),
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

    pub async fn insert_migration(
        &self,
        version: &str,
        script_path: &str,
        checksum: &str,
        execution_time_ms: i64,
        status: &str,
        tool_version: &str,
    ) -> Result<i64, SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                let row: (i64,) = sqlx::query_as(
                    "INSERT INTO schema_migration (version, script_path, checksum, execution_time_ms, status, tool_version) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"
                )
                .bind(version)
                .bind(script_path)
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
                    "INSERT INTO schema_migration (version, script_path, checksum, execution_time_ms, status, tool_version) VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(version)
                .bind(script_path)
                .bind(checksum)
                .bind(execution_time_ms)
                .bind(status)
                .bind(tool_version)
                .execute(pool)
                .await
                .map_err(|e| map_insert_error(e, version))?;

                let id: (i64,) = sqlx::query_as("SELECT id FROM schema_migration WHERE version = ? ORDER BY id DESC LIMIT 1")
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
                        "INSERT INTO schema_migration (version, script_path, checksum, execution_time_ms, status, tool_version) VALUES (@P1, @P2, @P3, @P4, @P5, @P6)",
                        &[&version, &script_path, &checksum, &(execution_time_ms as i32), &status, &tool_version],
                    )
                    .await
                    .map_err(|e| map_tiberius_insert_error(e, version))?;

                let result = client
                    .query("SELECT id FROM schema_migration WHERE version = @P1 ORDER BY id DESC", &[&version])
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let rows = result.into_first_result().await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let id: i64 = rows.into_iter().next()
                    .ok_or_else(|| SchemaInstallerError::Database("No inserted row found".to_string()))?
                    .get(0)
                    .ok_or_else(|| SchemaInstallerError::Database("Invalid id in inserted row".to_string()))?;

                Ok(id)
            }
        }
    }

    pub async fn update_migration_status(
        &self,
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
            AnyPool::Postgresql(pool) => {
                sqlx::query("UPDATE schema_migration SET status = $1, execution_time_ms = $2 WHERE id = $3")
                    .bind(status)
                    .bind(execution_time_ms)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                    .rows_affected()
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("UPDATE schema_migration SET status = ?, execution_time_ms = ? WHERE id = ?")
                    .bind(status)
                    .bind(execution_time_ms)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?
                    .rows_affected()
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        "UPDATE schema_migration SET status = @P1, execution_time_ms = @P2 WHERE id = @P3",
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

    pub async fn delete_failed_migrations(&self) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE status = $1")
                    .bind("failed")
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE status = ?")
                    .bind("failed")
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute("DELETE FROM schema_migration WHERE status = @P1", &[&"failed"])
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    /// Deletes "pending" tracking rows older than `older_than_seconds`. A row stays
    /// "pending" only for as long as the process that inserted it is actively running
    /// the migration (see `wait_for_slot` in migrator.rs); one still "pending" well past
    /// that window means the owning process crashed or was killed mid-migration and
    /// never got to mark it "success"/"failed", which would otherwise wedge every future
    /// `migrate()` call behind the lock-wait timeout forever with no way to recover.
    pub async fn delete_stale_pending_migrations(&self, older_than_seconds: i64) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query(
                    "DELETE FROM schema_migration WHERE status = $1 AND installed_at < now() - ($2 || ' seconds')::interval",
                )
                .bind("pending")
                .bind(older_than_seconds.to_string())
                .execute(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query(
                    "DELETE FROM schema_migration WHERE status = ? AND installed_at < datetime('now', '-' || ? || ' seconds')",
                )
                .bind("pending")
                .bind(older_than_seconds)
                .execute(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::SqlServer(client_mutex) => {
                let mut client = client_mutex.lock().await;
                client
                    .execute(
                        "DELETE FROM schema_migration WHERE status = @P1 AND installed_at < DATEADD(second, -@P2, SYSUTCDATETIME())",
                        &[&"pending", &(older_than_seconds as i32)],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
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
    pub async fn delete_failed_migration_by_version(&self, version: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE version = $1 AND status = $2")
                    .bind(version)
                    .bind("failed")
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE version = ? AND status = ?")
                    .bind(version)
                    .bind("failed")
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
                        &[&version, &"failed"],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn update_migration_checksum(
        &self,
        id: i64,
        checksum: &str,
    ) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query("UPDATE schema_migration SET checksum = $1 WHERE id = $2")
                    .bind(checksum)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("UPDATE schema_migration SET checksum = ? WHERE id = ?")
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
                        "UPDATE schema_migration SET checksum = @P1 WHERE id = @P2",
                        &[&checksum, &id],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
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
