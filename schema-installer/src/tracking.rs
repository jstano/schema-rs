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

    /// Idempotent repair for SQL Server `schema_migration` tables created before the
    /// NVARCHAR(MAX) -> bounded-length fix (see the comment on `schema_migration_ddl`'s
    /// `SqlServer` arm). `CREATE TABLE IF NOT EXISTS`-style guards never touch a table that
    /// already exists, so a database that ran `ensure_migration_table` before that fix landed
    /// is left with an unbounded `version` column forever, still hitting error 1919 on every
    /// startup - this brings such a table in line with the current DDL in place, without
    /// requiring the operator to run manual ALTER TABLE statements. Each column is only
    /// touched when it's still the old `NVARCHAR(MAX)`, so this is a no-op against a table
    /// that already has the current shape (including a fresh install).
    pub fn sqlserver_repair_ddl() -> String {
        r#"IF EXISTS (
    SELECT 1 FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = 'dbo' AND TABLE_NAME = 'schema_migration' AND COLUMN_NAME = 'version' AND CHARACTER_MAXIMUM_LENGTH = -1
)
BEGIN
    IF EXISTS (
        SELECT 1 FROM sys.key_constraints
        WHERE name = 'UQ_schema_migration_version' AND parent_object_id = OBJECT_ID('dbo.schema_migration')
    )
        ALTER TABLE dbo.schema_migration DROP CONSTRAINT UQ_schema_migration_version;

    ALTER TABLE dbo.schema_migration ALTER COLUMN version NVARCHAR(200) NOT NULL;
    ALTER TABLE dbo.schema_migration ADD CONSTRAINT UQ_schema_migration_version UNIQUE (version);
END

IF EXISTS (
    SELECT 1 FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = 'dbo' AND TABLE_NAME = 'schema_migration' AND COLUMN_NAME = 'script_path' AND CHARACTER_MAXIMUM_LENGTH = -1
)
    ALTER TABLE dbo.schema_migration ALTER COLUMN script_path NVARCHAR(1000) NOT NULL;

IF EXISTS (
    SELECT 1 FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = 'dbo' AND TABLE_NAME = 'schema_migration' AND COLUMN_NAME = 'checksum' AND CHARACTER_MAXIMUM_LENGTH = -1
)
    ALTER TABLE dbo.schema_migration ALTER COLUMN checksum NVARCHAR(64) NOT NULL;

IF EXISTS (
    SELECT 1 FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = 'dbo' AND TABLE_NAME = 'schema_migration' AND COLUMN_NAME = 'status' AND CHARACTER_MAXIMUM_LENGTH = -1
)
    ALTER TABLE dbo.schema_migration ALTER COLUMN status NVARCHAR(20) NOT NULL;

IF EXISTS (
    SELECT 1 FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = 'dbo' AND TABLE_NAME = 'schema_migration' AND COLUMN_NAME = 'tool_version' AND CHARACTER_MAXIMUM_LENGTH = -1
)
    ALTER TABLE dbo.schema_migration ALTER COLUMN tool_version NVARCHAR(50) NOT NULL;"#
            .to_string()
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

    #[test]
    fn test_sqlserver_repair_ddl_checks_all_originally_max_columns() {
        let ddl = SchemaMigrationDdl::sqlserver_repair_ddl();
        for column in ["version", "script_path", "checksum", "status", "tool_version"] {
            assert!(
                ddl.contains(&format!("COLUMN_NAME = '{}'", column)),
                "repair DDL should check column '{}' for the old NVARCHAR(MAX) type",
                column
            );
        }
    }

    #[test]
    fn test_sqlserver_repair_ddl_fixes_version_column_and_constraint() {
        let ddl = SchemaMigrationDdl::sqlserver_repair_ddl();
        assert!(ddl.contains("ALTER TABLE dbo.schema_migration DROP CONSTRAINT UQ_schema_migration_version"));
        assert!(ddl.contains("ALTER TABLE dbo.schema_migration ALTER COLUMN version NVARCHAR(200) NOT NULL"));
        assert!(ddl.contains("ALTER TABLE dbo.schema_migration ADD CONSTRAINT UQ_schema_migration_version UNIQUE (version)"));
    }

    #[test]
    fn test_sqlserver_repair_ddl_bounds_remaining_columns_to_match_fresh_ddl() {
        let ddl = SchemaMigrationDdl::sqlserver_repair_ddl();
        assert!(ddl.contains("ALTER COLUMN script_path NVARCHAR(1000) NOT NULL"));
        assert!(ddl.contains("ALTER COLUMN checksum NVARCHAR(64) NOT NULL"));
        assert!(ddl.contains("ALTER COLUMN status NVARCHAR(20) NOT NULL"));
        assert!(ddl.contains("ALTER COLUMN tool_version NVARCHAR(50) NOT NULL"));
    }
}
