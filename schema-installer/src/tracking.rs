use schema_sql_generator::common::generator_type::GeneratorType;

pub struct TrackingTableDdl;

impl TrackingTableDdl {
    pub fn database_version_ddl(database_type: &GeneratorType) -> String {
        match database_type {
            GeneratorType::Postgres => {
                "CREATE TABLE IF NOT EXISTS databaseversion (\n  version VARCHAR(10) PRIMARY KEY\n);\n".to_string()
            }
            GeneratorType::SqlServer => {
                "IF NOT EXISTS (SELECT * FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA='dbo' AND TABLE_NAME='databaseversion')\n  CREATE TABLE dbo.databaseversion (\n    version NVARCHAR(10) PRIMARY KEY\n  );\n".to_string()
            }
            GeneratorType::Sqlite => {
                "CREATE TABLE IF NOT EXISTS databaseversion (\n  version TEXT PRIMARY KEY\n);\n".to_string()
            }
        }
    }

    pub fn upgrade_log_ddl(database_type: &GeneratorType) -> String {
        match database_type {
            GeneratorType::Postgres => {
                "CREATE TABLE IF NOT EXISTS databaseupgradelog (\n  id SERIAL PRIMARY KEY,\n  start_datetime TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,\n  end_datetime TIMESTAMP,\n  changelog_name VARCHAR(200),\n  error TEXT\n);\n".to_string()
            }
            GeneratorType::SqlServer => {
                "IF NOT EXISTS (SELECT * FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA='dbo' AND TABLE_NAME='databaseupgradelog')\n  CREATE TABLE dbo.databaseupgradelog (\n    id INT IDENTITY(1,1) PRIMARY KEY,\n    start_datetime DATETIME NOT NULL DEFAULT GETDATE(),\n    end_datetime DATETIME,\n    changelog_name NVARCHAR(200),\n    error NVARCHAR(MAX)\n  );\n".to_string()
            }
            GeneratorType::Sqlite => {
                "CREATE TABLE IF NOT EXISTS databaseupgradelog (\n  id INTEGER PRIMARY KEY AUTOINCREMENT,\n  start_datetime DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,\n  end_datetime DATETIME,\n  changelog_name TEXT,\n  error TEXT\n);\n".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_ddl() {
        let ddl = TrackingTableDdl::database_version_ddl(&GeneratorType::Postgres);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS databaseversion"));
        assert!(ddl.contains("VARCHAR(10)"));
    }

    #[test]
    fn test_sqlserver_ddl() {
        let ddl = TrackingTableDdl::database_version_ddl(&GeneratorType::SqlServer);
        assert!(ddl.contains("dbo.databaseversion"));
        assert!(ddl.contains("NVARCHAR(10)"));
    }

    #[test]
    fn test_sqlite_ddl() {
        let ddl = TrackingTableDdl::database_version_ddl(&GeneratorType::Sqlite);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS databaseversion"));
        assert!(ddl.contains("TEXT PRIMARY KEY"));
    }
}
