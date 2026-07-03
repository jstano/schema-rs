use schema_model::model::types::DatabaseType;

use crate::migration_generator::MigrationGenerator;
use crate::postgresql::PostgresqlMigrationGenerator;
use crate::sqlite::SqliteMigrationGenerator;
use crate::sqlserver::SqlServerMigrationGenerator;

pub fn create_generator(db_type: DatabaseType) -> Box<dyn MigrationGenerator> {
    match db_type {
        DatabaseType::Postgresql => Box::new(PostgresqlMigrationGenerator),
        DatabaseType::Sqlite => Box::new(SqliteMigrationGenerator),
        DatabaseType::SqlServer => Box::new(SqlServerMigrationGenerator),
    }
}
