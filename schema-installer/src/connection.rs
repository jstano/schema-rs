use sqlx::{postgres::PgPoolOptions, sqlite::SqlitePoolOptions, Pool, Postgres, Sqlite, AnyPool as SqlxAnyPool, Row};
use schema_sql_generator::common::generator_type::GeneratorType;
use crate::error::SchemaInstallerError;

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
}
