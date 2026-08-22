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
            .map_err(SchemaInstallerError::Io)?;
        let database_model = parse_database_xml(&schema_content)
            .map_err(SchemaInstallerError::Parse)?;

        // Catches dangling relation targets and enum-type references up front, with a
        // clear message - several generator code paths panic on a reference that
        // doesn't resolve, on the assumption the model was already validated.
        let validation_errors = database_model.validate();
        if !validation_errors.is_empty() {
            return Err(SchemaInstallerError::ValidationFailed(validation_errors.join("\n")));
        }

        // Generate SQL by writing to temp file
        // (PrintWriter's BufWriter makes it difficult to extract bytes in memory)
        let temp_file = std::env::temp_dir().join(temp_install_file_name());
        let file = std::fs::File::create(&temp_file)
            .map_err(SchemaInstallerError::Io)?;

        let writer_temp = PrintWriter::new(Box::new(file));
        let generate_options = GenerateOptions {
            database_model: Rc::new(database_model),
            writer: Rc::new(RefCell::new(writer_temp)),
            boolean_mode: config.boolean_mode,
            foreign_key_mode: config.foreign_key_mode,
            output_mode: OutputMode::All,
            target_postgres_version: 17,
            emit_postgres_extensions: true,
            extension_check_user: None,
        };

        config.database_type.generate(generate_options);

        let sql = std::fs::read_to_string(&temp_file)
            .map_err(SchemaInstallerError::Io)?;

        let _ = std::fs::remove_file(&temp_file);

        // Record migration under a fixed, reserved version so it can never collide
        // with real migration versions (which start at V1+).
        let install_version = crate::migration::RESERVED_INSTALL_VERSION;
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
                println!("Schema installed successfully.");
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

/// Builds a temp filename for the generated install SQL. Combines the full (not just
/// sub-second) nanosecond timestamp, the process id, and a per-process monotonic
/// counter: `subsec_nanos()` alone (discarding the seconds component, with no PID or
/// counter) could collide between two concurrent `install()` calls on the same host
/// started moments apart, letting one overwrite or read the other's generated SQL
/// mid-flight - and the timestamp alone isn't safe either, since two calls on a coarser
/// clock (or simply fast enough back-to-back) can still land on the same nanosecond
/// reading. The counter guarantees uniqueness within a process regardless of clock
/// resolution; the PID guarantees it across processes.
fn temp_install_file_name() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("schema_install_temp_{}_{}_{}.sql", std::process::id(), nanos, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_install_file_name_embeds_the_process_id() {
        let name = temp_install_file_name();
        assert!(name.starts_with(&format!("schema_install_temp_{}_", std::process::id())));
        assert!(name.ends_with(".sql"));
    }

    #[test]
    fn temp_install_file_name_is_unique_across_calls() {
        // A cheap proxy for the concurrency fix: two calls (even back-to-back, so the
        // PID is identical) must not produce the same filename, since the nanosecond
        // timestamp component differs.
        let names: std::collections::HashSet<String> = (0..20).map(|_| temp_install_file_name()).collect();
        assert_eq!(names.len(), 20, "expected all 20 generated names to be unique");
    }
}
