use crate::error::SchemaInstallerError;
use crate::migration::AppliedMigration;
use crate::tracking::SchemaMigrationDdl;
use schema_sql_generator::common::generator_type::GeneratorType;
use sqlx::{postgres::PgPoolOptions, sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, Postgres, Sqlite};
use std::str::FromStr;
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
                let options = SqliteConnectOptions::from_str(connection_string)
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?
                    .create_if_missing(true);

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
        self.execute_sql(&ddl).await
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
                    let installed_at: chrono::DateTime<chrono::Utc> = row.get(5).ok_or_else(|| SchemaInstallerError::Database("Missing installed_at".to_string()))?;
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
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
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
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

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
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

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
        match self {
            AnyPool::Postgresql(pool) => {
                sqlx::query("UPDATE schema_migration SET status = $1, execution_time_ms = $2 WHERE id = $3")
                    .bind(status)
                    .bind(execution_time_ms)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("UPDATE schema_migration SET status = ?, execution_time_ms = ? WHERE id = ?")
                    .bind(status)
                    .bind(execution_time_ms)
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
                        "UPDATE schema_migration SET status = @P1, execution_time_ms = @P2 WHERE id = @P3",
                        &[&status, &(execution_time_ms as i32), &id],
                    )
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
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
