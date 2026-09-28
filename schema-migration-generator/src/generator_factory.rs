use schema_model::model::types::DatabaseType;

use crate::migration_generator::MigrationGenerator;
use crate::postgresql::PostgresqlMigrationGenerator;
use crate::sqlite::SqliteMigrationGenerator;
use crate::sqlserver::SqlServerMigrationGenerator;

pub fn create_generator(db_type: DatabaseType) -> Box<dyn MigrationGenerator> {
    create_generator_with_versions(db_type, 0, 0)
}

/// Like `create_generator`, but with target dialect versions threaded through - see
/// `PostgresqlMigrationGenerator::target_postgres_version` / `SqlServerMigrationGenerator::target_sqlserver_version`.
/// `target_postgres_version`/`target_sqlserver_version` are each ignored except by their own
/// dialect (and both are ignored for `DatabaseType::Sqlite`).
pub fn create_generator_with_versions(
    db_type: DatabaseType,
    target_postgres_version: u32,
    target_sqlserver_version: u32,
) -> Box<dyn MigrationGenerator> {
    match db_type {
        DatabaseType::Postgresql => Box::new(PostgresqlMigrationGenerator::new(target_postgres_version)),
        DatabaseType::Sqlite => Box::new(SqliteMigrationGenerator),
        DatabaseType::SqlServer => Box::new(SqlServerMigrationGenerator::new(target_sqlserver_version)),
    }
}
