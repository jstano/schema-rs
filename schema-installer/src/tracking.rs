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
    tool_version TEXT NOT NULL,
    CONSTRAINT schema_migration_version_key UNIQUE (version)
);"#
                    .to_string()
            }
            GeneratorType::SqlServer => {
                // `version` must be a bounded type - SQL Server forbids LOB types (NVARCHAR(MAX)
                // included) as index/constraint key columns (Msg 1919), so
                // `UNIQUE (version)` fails immediately on an unbounded column. The other
                // NVARCHAR(MAX) columns aren't key columns and would work either way, but are
                // bounded too for consistency and to match their actual content (a SHA-256 hex
                // checksum is always 64 chars, `status` is one of a handful of short words, ...).
                r#"IF NOT EXISTS (SELECT * FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA='dbo' AND TABLE_NAME='schema_migration')
BEGIN
  CREATE TABLE dbo.schema_migration (
    id BIGINT IDENTITY(1,1) PRIMARY KEY,
    version NVARCHAR(200) NOT NULL,
    script_path NVARCHAR(1000) NOT NULL,
    checksum NVARCHAR(64) NOT NULL,
    execution_time_ms INT NOT NULL,
    installed_at DATETIME2 NOT NULL DEFAULT SYSUTCDATETIME(),
    status NVARCHAR(20) NOT NULL,
    tool_version NVARCHAR(50) NOT NULL,
    CONSTRAINT UQ_schema_migration_version UNIQUE (version)
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
    tool_version TEXT NOT NULL,
    UNIQUE (version)
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
        assert!(ddl.contains("UNIQUE (version)"));
    }

    #[test]
    fn test_postgres_schema_migration_ddl_has_unique_version() {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::Postgresql);
        assert!(ddl.contains("UNIQUE (version)"));
    }

    #[test]
    fn test_sqlserver_schema_migration_ddl_has_unique_version() {
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::SqlServer);
        assert!(ddl.contains("UNIQUE (version)"));
    }

    #[test]
    fn test_sqlserver_schema_migration_ddl_has_no_lob_columns() {
        // Regression test for C6: SQL Server forbids LOB types (NVARCHAR(MAX) included) as
        // index/constraint key columns (Msg 1919) - `UNIQUE (version)` against an unbounded
        // `version` column made `ensure_migration_table` fail immediately on every SQL Server
        // install.
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::SqlServer);
        assert!(!ddl.contains("NVARCHAR(MAX)"));
        assert!(ddl.contains("version NVARCHAR(200)"));
    }

    #[test]
    fn test_sqlserver_schema_migration_ddl_uses_utc_default() {
        // `installed_at` is read back as a naive (timezone-less) DATETIME2 and treated as UTC
        // (see `AnyPool::get_applied_migrations`) - the default must actually produce a UTC
        // value, not `GETDATE()`'s server-local time, or `installed_at` would silently be off
        // by the server's UTC offset.
        let ddl = SchemaMigrationDdl::schema_migration_ddl(&GeneratorType::SqlServer);
        assert!(ddl.contains("DEFAULT SYSUTCDATETIME()"));
        assert!(!ddl.contains("GETDATE()"));
    }
}
