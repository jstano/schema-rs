use schema_sql_generator::common::generator_type::GeneratorType;

pub struct SchemaMigrationDdl;

impl SchemaMigrationDdl {
    pub fn schema_migration_ddl(database_type: &GeneratorType) -> String {
        match database_type {
            GeneratorType::Postgresql => {
                r#"CREATE TABLE IF NOT EXISTS schema_migration (
    id BIGSERIAL PRIMARY KEY,
    version TEXT NOT NULL,
    script_path TEXT NOT NULL,
    checksum TEXT NOT NULL,
    execution_time_ms INT NOT NULL,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL,
    tool_version TEXT NOT NULL
);"#
                    .to_string()
            }
            GeneratorType::SqlServer => {
                r#"IF NOT EXISTS (SELECT * FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA='dbo' AND TABLE_NAME='schema_migration')
BEGIN
  CREATE TABLE dbo.schema_migration (
    id BIGINT IDENTITY(1,1) PRIMARY KEY,
    version NVARCHAR(MAX) NOT NULL,
    script_path NVARCHAR(MAX) NOT NULL,
    checksum NVARCHAR(MAX) NOT NULL,
    execution_time_ms INT NOT NULL,
    installed_at DATETIME2 NOT NULL DEFAULT GETDATE(),
    status NVARCHAR(MAX) NOT NULL,
    tool_version NVARCHAR(MAX) NOT NULL
  );
END"#
                    .to_string()
            }
            GeneratorType::Sqlite => {
                r#"CREATE TABLE IF NOT EXISTS schema_migration (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL,
    script_path TEXT NOT NULL,
    checksum TEXT NOT NULL,
    execution_time_ms INTEGER NOT NULL,
    installed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status TEXT NOT NULL,
    tool_version TEXT NOT NULL
);"#
                    .to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_schema_migration_ddl() {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::Postgresql);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS schema_migration"));
        assert!(ddl.contains("BIGSERIAL PRIMARY KEY"));
    }

    #[test]
    fn test_sqlserver_schema_migration_ddl() {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::SqlServer);
        assert!(ddl.contains("dbo.schema_migration"));
        assert!(ddl.contains("BIGINT IDENTITY(1,1)"));
    }

    #[test]
    fn test_sqlite_schema_migration_ddl() {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::Sqlite);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS schema_migration"));
        assert!(ddl.contains("INTEGER PRIMARY KEY AUTOINCREMENT"));
    }
}
