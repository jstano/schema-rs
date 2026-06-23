use sqlx::{postgres::PgPoolOptions, sqlite::SqlitePoolOptions, Pool, Postgres, Sqlite, AnyPool as SqlxAnyPool, Row};
use schema_sql_generator::common::generator_type::GeneratorType;
use crate::error::SchemaInstallerError;
use crate::migration::AppliedMigration;
use crate::tracking::SchemaMigrationDdl;

pub enum AnyPool {
    Postgres(Pool<Postgres>),
    Sqlite(Pool<Sqlite>),
    Any(SqlxAnyPool),
}

impl AnyPool {
    pub async fn connect(database_type: &GeneratorType, connection_string: &str) -> Result<Self, SchemaInstallerError> {
        match database_type {
            GeneratorType::Postgres => {
                let pool = PgPoolOptions::new()
                    .max_connections(5)
                    .connect(connection_string)
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?;
                Ok(AnyPool::Postgres(pool))
            }
            GeneratorType::Sqlite => {
                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect(connection_string)
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?;
                Ok(AnyPool::Sqlite(pool))
            }
            GeneratorType::SqlServer => {
                let pool = sqlx::any::AnyPoolOptions::new()
                    .max_connections(5)
                    .connect(connection_string)
                    .await
                    .map_err(|e| SchemaInstallerError::Connection(e.to_string()))?;
                Ok(AnyPool::Any(pool))
            }
        }
    }

    pub async fn execute_sql(&self, sql: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
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
            AnyPool::Any(pool) => {
                sqlx::query(sql)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Execution(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn query_version(&self) -> Result<Option<String>, SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
                let row: Option<(String,)> = sqlx::query_as("SELECT version FROM databaseversion LIMIT 1")
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(row.map(|(v,)| v))
            }
            AnyPool::Sqlite(pool) => {
                let row: Option<(String,)> = sqlx::query_as("SELECT version FROM databaseversion LIMIT 1")
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(row.map(|(v,)| v))
            }
            AnyPool::Any(pool) => {
                let row = sqlx::query("SELECT version FROM databaseversion")
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(row.and_then(|r| r.try_get(0).ok()))
            }
        }
    }

    pub async fn insert_version(&self, version: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
                sqlx::query("INSERT INTO databaseversion (version) VALUES ($1)")
                    .bind(version)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("INSERT INTO databaseversion (version) VALUES (?1)")
                    .bind(version)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Any(pool) => {
                sqlx::query("INSERT INTO databaseversion (version) VALUES (?)")
                    .bind(version)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn log_upgrade_start(&self, changelog_name: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
                sqlx::query("INSERT INTO databaseupgradelog (changelog_name) VALUES ($1)")
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("INSERT INTO databaseupgradelog (changelog_name) VALUES (?1)")
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Any(pool) => {
                sqlx::query("INSERT INTO databaseupgradelog (changelog_name) VALUES (?)")
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn log_upgrade_error(&self, changelog_name: &str, error: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
                sqlx::query("UPDATE databaseupgradelog SET end_datetime = CURRENT_TIMESTAMP, error = $1 WHERE changelog_name = $2")
                    .bind(error)
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("UPDATE databaseupgradelog SET end_datetime = CURRENT_TIMESTAMP, error = ?1 WHERE changelog_name = ?2")
                    .bind(error)
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Any(pool) => {
                sqlx::query("UPDATE databaseupgradelog SET error = ? WHERE changelog_name = ?")
                    .bind(error)
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn log_upgrade_success(&self, changelog_name: &str) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
                sqlx::query("UPDATE databaseupgradelog SET end_datetime = CURRENT_TIMESTAMP WHERE changelog_name = $1")
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Sqlite(pool) => {
                sqlx::query("UPDATE databaseupgradelog SET end_datetime = CURRENT_TIMESTAMP WHERE changelog_name = ?1")
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
            AnyPool::Any(pool) => {
                sqlx::query("UPDATE databaseupgradelog SET end_datetime = CURRENT_TIMESTAMP WHERE changelog_name = ?")
                    .bind(changelog_name)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
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
            AnyPool::Postgres(pool) => {
                let rows: Vec<(i64, String, String, String, i32, String, String, String)> =
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
                                installed_at,
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
            AnyPool::Any(pool) => {
                let rows = sqlx::query(
                    "SELECT id, version, script_path, checksum, execution_time_ms, installed_at, status, tool_version FROM schema_migration ORDER BY installed_at"
                )
                .fetch_all(pool)
                .await
                .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                let migrations = rows
                    .into_iter()
                    .map(|row| {
                        let id: i64 = row.try_get(0).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let version: String = row.try_get(1).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let script_path: String = row.try_get(2).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let checksum: String = row.try_get(3).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let execution_time_ms: i64 = row.try_get(4).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let installed_at: String = row.try_get(5).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let status: String = row.try_get(6).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                        let tool_version: String = row.try_get(7).map_err(|e| SchemaInstallerError::Database(e.to_string()))?;

                        Ok(AppliedMigration {
                            id,
                            version,
                            script_path,
                            checksum,
                            execution_time_ms,
                            installed_at,
                            status,
                            tool_version,
                        })
                    })
                    .collect::<Result<Vec<_>, SchemaInstallerError>>()?;

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
            AnyPool::Postgres(pool) => {
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
            AnyPool::Any(pool) => {
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
                Ok(0)
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
            AnyPool::Postgres(pool) => {
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
            AnyPool::Any(pool) => {
                sqlx::query("UPDATE schema_migration SET status = ?, execution_time_ms = ? WHERE id = ?")
                    .bind(status)
                    .bind(execution_time_ms)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub async fn delete_failed_migrations(&self) -> Result<(), SchemaInstallerError> {
        match self {
            AnyPool::Postgres(pool) => {
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
            AnyPool::Any(pool) => {
                sqlx::query("DELETE FROM schema_migration WHERE status = ?")
                    .bind("failed")
                    .execute(pool)
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
            AnyPool::Postgres(pool) => {
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
            AnyPool::Any(pool) => {
                sqlx::query("UPDATE schema_migration SET checksum = ? WHERE id = ?")
                    .bind(checksum)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| SchemaInstallerError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }
}
