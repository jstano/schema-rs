use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use schema_parser::parse_database_xml;
use schema_sql_generator::common::generate_options::GenerateOptions;
use schema_sql_generator::common::print_writer::PrintWriter;
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_sql_generator::common::output_mode::OutputMode;
use crate::config::SchemaInstallerConfig;
use crate::connection::AnyPool;
use crate::error::SchemaInstallerError;
use crate::tracking::TrackingTableDdl;

pub struct SchemaInstaller;

impl SchemaInstaller {
    pub async fn install(config: &SchemaInstallerConfig) -> Result<(), SchemaInstallerError> {
        // Connect to database
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        // Create tracking tables if they don't exist
        Self::ensure_tracking_tables(&pool, &config.database_type).await?;

        // Check if already installed
        if Self::check_if_installed(&pool).await? {
            println!("Schema is already installed. Skipping installation.");
            return Ok(());
        }

        // Parse schema
        let schema_file_str = config.schema_file.to_str()
            .ok_or_else(|| SchemaInstallerError::SchemaFileNotFound("Invalid path".to_string()))?;
        let schema_content = fs::read_to_string(schema_file_str)
            .map_err(|e| SchemaInstallerError::Io(e))?;
        let database_model = parse_database_xml(&schema_content)
            .map_err(|e| SchemaInstallerError::Parse(e))?;

        // Get schema version
        let version = database_model.version()
            .map(|v| format!("{}.{}.{}", v.major_version(), v.minor_version(), v.patch_version()))
            .unwrap_or_else(|| "1.0.0".to_string());

        // Generate SQL by writing to temp file
        // (PrintWriter's BufWriter makes it difficult to extract bytes in memory)
        let temp_file = std::env::temp_dir().join("schema_install_temp.sql");
        let file = std::fs::File::create(&temp_file)
            .map_err(|e| SchemaInstallerError::Io(e))?;

        let writer_temp = PrintWriter::new(Box::new(file));
        let generate_options = GenerateOptions {
            database_model: Rc::new(database_model),
            writer: Rc::new(RefCell::new(writer_temp)),
            boolean_mode: config.boolean_mode.clone(),
            foreign_key_mode: config.foreign_key_mode.clone(),
            output_mode: OutputMode::All,
            target_postgres_version: 17,
        };

        (&config.database_type).generate(generate_options);

        let sql = std::fs::read_to_string(&temp_file)
            .map_err(|e| SchemaInstallerError::Io(e))?;

        let _ = std::fs::remove_file(&temp_file);

        // Log upgrade start
        let changelog_name = format!("V1__install_schema_{}.sql", version);
        pool.log_upgrade_start(&changelog_name).await?;

        // Execute SQL statements
        match Self::execute_sql_script(&pool, &config.database_type, &sql).await {
            Ok(_) => {
                // Update version
                pool.insert_version(&version).await?;

                // Log upgrade success
                pool.log_upgrade_success(&changelog_name).await?;

                println!("Schema installed successfully. Version: {}", version);
                Ok(())
            }
            Err(e) => {
                // Log error
                let _ = pool.log_upgrade_error(&changelog_name, &e.to_string()).await;
                Err(e)
            }
        }
    }

    pub async fn is_installed(config: &SchemaInstallerConfig) -> Result<bool, SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;
        Self::check_if_installed(&pool).await
    }

    pub async fn get_installed_version(config: &SchemaInstallerConfig) -> Result<Option<String>, SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;
        pool.query_version().await
    }

    async fn check_if_installed(pool: &AnyPool) -> Result<bool, SchemaInstallerError> {
        match pool.query_version().await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => {
                // Table might not exist yet, which is fine
                if e.to_string().contains("does not exist") || e.to_string().contains("no such table") {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn ensure_tracking_tables(pool: &AnyPool, database_type: &GeneratorType) -> Result<(), SchemaInstallerError> {
        let version_ddl = TrackingTableDdl::database_version_ddl(database_type);
        let upgrade_log_ddl = TrackingTableDdl::upgrade_log_ddl(database_type);

        pool.execute_sql(&version_ddl).await?;
        pool.execute_sql(&upgrade_log_ddl).await?;

        Ok(())
    }

    async fn execute_sql_script(
        pool: &AnyPool,
        database_type: &GeneratorType,
        sql: &str,
    ) -> Result<(), SchemaInstallerError> {
        // Split SQL statements based on database type
        let delimiter = match database_type {
            GeneratorType::SqlServer => "GO",
            _ => ";",
        };

        let statements: Vec<&str> = sql.split(delimiter)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for statement in statements {
            pool.execute_sql(statement).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sql_script_splitting() {
        // Test PostgreSQL delimiter
        let sql_pg = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
        let statements: Vec<&str> = sql_pg.split(";")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(statements.len(), 2);

        // Test SQL Server delimiter
        let sql_mssql = "CREATE TABLE t1 (id INT)\nGO\nCREATE TABLE t2 (id INT)\nGO";
        let statements: Vec<&str> = sql_mssql.split("GO")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(statements.len(), 2);
    }
}
