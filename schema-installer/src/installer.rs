use crate::config::SchemaInstallerConfig;
use crate::connection::AnyPool;
use crate::error::SchemaInstallerError;
use schema_parser::parse_database_xml;
use schema_sql_generator::common::generate_options::GenerateOptions;
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_sql_generator::common::output_mode::OutputMode;
use schema_sql_generator::common::print_writer::PrintWriter;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

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
        let schema_file = config.schema_file.as_ref()
            .ok_or_else(|| SchemaInstallerError::InvalidConfiguration("schema_file required for install command".to_string()))?;
        let schema_file_str = schema_file.to_str()
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
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let temp_file = std::env::temp_dir().join(format!("schema_install_temp_{}.sql", nanos));
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

        // Record migration under a fixed, reserved version so it can never collide
        // with real migration versions (which start at V1+).
        let install_version = "0";
        let script_name = "V0__install_schema.sql";
        let checksum = crate::migration::compute_checksum(&sql);
        let tool_version = env!("CARGO_PKG_VERSION");

        let migration_id = pool
            .insert_migration(install_version, script_name, &checksum, 0, "pending", tool_version)
            .await?;

        // Execute SQL statements
        let start = std::time::Instant::now();
        match Self::execute_sql_script(&pool, &config.database_type, &sql).await {
            Ok(_) => {
                let elapsed_ms = start.elapsed().as_millis() as i64;
                pool.update_migration_status(migration_id, "success", elapsed_ms)
                    .await?;
                println!("Schema installed successfully. Version: {}", version);
                Ok(())
            }
            Err(e) => {
                let elapsed_ms = start.elapsed().as_millis() as i64;
                pool.update_migration_status(migration_id, "failed", elapsed_ms)
                    .await?;
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
        match pool.get_applied_migrations().await {
            Ok(migrations) => {
                let latest = migrations
                    .iter()
                    .filter(|m| m.status == "success")
                    .max_by(|a, b| {
                        crate::migration::compare_versions(&a.version, &b.version)
                    });
                Ok(latest.map(|m| m.version.clone()))
            }
            Err(e) => {
                // Table might not exist yet, which is fine
                if e.to_string().contains("does not exist") || e.to_string().contains("no such table") {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn check_if_installed(pool: &AnyPool) -> Result<bool, SchemaInstallerError> {
        match pool.get_applied_migrations().await {
            Ok(migrations) => Ok(migrations.iter().any(|m| m.status == "success")),
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
        pool.ensure_migration_table(database_type).await?;
        Ok(())
    }

    async fn execute_sql_script(
        pool: &AnyPool,
        database_type: &GeneratorType,
        sql: &str,
    ) -> Result<(), SchemaInstallerError> {
        for statement in crate::sql_split::split_sql_statements(sql, database_type) {
            pool.execute_sql(&statement).await?;
        }

        Ok(())
    }
}
