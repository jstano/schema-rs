use crate::config::SchemaInstallerConfig;
use crate::connection::AnyPool;
use crate::error::SchemaInstallerError;
use crate::migration::{MigrationSource, MigrationStatus, compare_versions, compute_checksum};
use crate::migrator::Migrator;
use schema_parser::parse_database_xml;
use schema_sql_generator::common::generate_options::GenerateOptions;
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_sql_generator::common::output_mode::OutputMode;
use schema_sql_generator::common::print_writer::PrintWriter;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

/// Flyway-style baseline: marks every migration in `source` up to and including
/// `version` as already applied by `install`, without running its SQL. See
/// `SchemaInstaller::install_with_baseline`.
pub struct Baseline<'a> {
    pub version: &'a str,
    pub source: &'a dyn MigrationSource,
}

pub struct SchemaInstaller;

impl SchemaInstaller {
    pub async fn install(config: &SchemaInstallerConfig) -> Result<(), SchemaInstallerError> {
        Self::install_with_baseline(config, None).await
    }

    /// Like `install`, but when `baseline` is given, also marks every migration from
    /// `baseline.source` up to and including `baseline.version` as already applied
    /// (Flyway's `baselineVersion`) - without running their SQL. This is the missing
    /// piece for adopting the migration workflow after an `install`: without it,
    /// `Migrator::migrate` has no way to know the freshly-generated schema already
    /// contains everything through `baseline.version`, and replays that SQL against a
    /// schema that already has it (L20).
    pub async fn install_with_baseline(
        config: &SchemaInstallerConfig,
        baseline: Option<Baseline<'_>>,
    ) -> Result<(), SchemaInstallerError> {
        // Connect to database
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        // Create tracking tables if they don't exist
        Self::ensure_tracking_tables(&pool, &config.database_type).await?;

        // Check if already installed
        if Self::check_if_installed(&pool).await? {
            println!("Schema is already installed. Skipping installation.");
            return Ok(());
        }

        // Parse schema
        let schema_file = config.schema_file.as_ref().ok_or_else(|| {
            SchemaInstallerError::InvalidConfiguration(
                "schema_file required for install command".to_string(),
            )
        })?;
        let schema_file_str = schema_file
            .to_str()
            .ok_or_else(|| SchemaInstallerError::SchemaFileNotFound("Invalid path".to_string()))?;
        let schema_content =
            fs::read_to_string(schema_file_str).map_err(SchemaInstallerError::Io)?;
        let database_model =
            parse_database_xml(&schema_content).map_err(SchemaInstallerError::Parse)?;

        // Catches dangling relation targets and enum-type references up front, with a
        // clear message - several generator code paths panic on a reference that
        // doesn't resolve, on the assumption the model was already validated.
        let validation_errors = database_model.validate();
        if !validation_errors.is_empty() {
            return Err(SchemaInstallerError::ValidationFailed(
                validation_errors.join("\n"),
            ));
        }

        // Dialect-specific checks `validate()` can't make on its own, since it has no
        // notion of which dialect is about to render this model - one schema.xml is meant
        // to target all three databases (H23).
        let dialect_errors = config.database_type.validate_for_dialect(&database_model);
        if !dialect_errors.is_empty() {
            return Err(SchemaInstallerError::ValidationFailed(
                dialect_errors.join("\n"),
            ));
        }

        // Generate SQL into an in-memory buffer. This used to go through a file at a
        // predictable path in the shared temp dir (L9): another local user could pre-create
        // or symlink it and have their DDL executed with migration privileges. A buffer has
        // no path to hijack, and nothing for a failed cleanup to leave behind.
        let buffer = SharedBuffer::default();
        let writer = Rc::new(RefCell::new(PrintWriter::new(Box::new(buffer.clone()))));
        let generate_options = GenerateOptions {
            database_model: Rc::new(database_model),
            writer: writer.clone(),
            boolean_mode: config.boolean_mode,
            foreign_key_mode: config.foreign_key_mode,
            output_mode: OutputMode::All,
            target_postgres_version: 17,
            emit_postgres_extensions: true,
            extension_check_user: None,
            target_sqlserver_version: 0,
        };

        config.database_type.generate(generate_options);

        // `PrintWriter` buffers internally and is otherwise only flushed by `Drop`, which
        // swallows errors - an unflushed tail would be checksummed and executed as if the
        // script were complete. Flush explicitly so a failure is loud, not truncated SQL.
        writer.borrow_mut().flush();

        let sql = String::from_utf8(buffer.take()).map_err(|e| {
            SchemaInstallerError::Execution(format!("generated SQL is not valid UTF-8: {e}"))
        })?;

        // Record migration under a fixed, reserved version so it can never collide
        // with real migration versions (which start at V1+).
        let install_version = crate::migration::RESERVED_INSTALL_VERSION;
        let script_name = "V0__install_schema.sql";
        let checksum = crate::migration::compute_checksum(&sql);
        let tool_version = env!("CARGO_PKG_VERSION");

        // Clear a leftover "failed" row from a previous, non-transactional install
        // attempt before claiming a fresh "pending" one - otherwise it collides with
        // the insert below (UNIQUE (version)) and surfaces as a bogus
        // `ConcurrentMigrationDetected` with no concurrency involved (H14).
        pool.delete_failed_migration_by_version(install_version)
            .await?;

        let migration_id = pool
            .insert_migration(
                install_version,
                script_name,
                "install schema",
                &checksum,
                0,
                MigrationStatus::Pending.as_str(),
                tool_version,
            )
            .await?;

        // Execute all statements and record "success" as one transaction (H14): the
        // previous loop ran each statement autocommit via `execute_sql`, so a failure
        // partway through (e.g. statement 23 of 40) left every prior table behind with
        // nothing rolled back, and re-running `install` then failed trying to re-insert
        // the reserved version-0 tracking row over the leftovers. Reusing the same
        // `execute_migration_transactional` the migration path uses (see H9) means a
        // failure rolls back the entire script, leaving nothing behind to retry against.
        let start = std::time::Instant::now();
        let result = match crate::sql_split::split_sql_statements(&sql, &config.database_type) {
            Ok(statements) => {
                pool.execute_migration_transactional(&statements, migration_id)
                    .await
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(_) => {
                println!("Schema installed successfully.");

                if let Some(baseline) = baseline {
                    Self::record_baseline(&pool, baseline, tool_version).await?;
                }

                Ok(())
            }
            Err(e) => {
                let elapsed_ms = start.elapsed().as_millis() as i64;
                pool.update_migration_status(
                    migration_id,
                    MigrationStatus::Failed.as_str(),
                    elapsed_ms,
                )
                .await?;
                Err(e)
            }
        }
    }

    /// Install-if-needed, then migrate: the combined startup flow for an application that
    /// owns its schema. A database with no migration history is installed from
    /// `config.schema_file` and baselined at the newest migration in `source` (the XML is
    /// the source of truth for a fresh database, so replaying history over it would fail);
    /// then `Migrator::migrate_to_target` runs, which applies only what is newer than the
    /// baseline plus any repeatable (`R__`) migrations. An already-installed database, or
    /// one built up by `migrate` alone, skips straight to the migrate step.
    pub async fn install_or_migrate(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
        target: Option<&str>,
    ) -> Result<(), SchemaInstallerError> {
        if !Self::has_migration_history(config).await? {
            let latest = source
                .migrations()?
                .into_iter()
                .map(|m| m.version)
                .max_by(|a, b| compare_versions(a, b));
            let baseline = latest.as_deref().map(|version| Baseline {
                version,
                source: source.as_ref(),
            });
            Self::install_with_baseline(config, baseline).await?;
        }
        Migrator::migrate_to_target(config, source, target).await
    }

    /// True when the database has any successfully applied migration, including the
    /// `install` row. A failed or pending-only history counts as none, so a half-finished
    /// install is retried rather than skipped.
    async fn has_migration_history(
        config: &SchemaInstallerConfig,
    ) -> Result<bool, SchemaInstallerError> {
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;
        Self::ensure_tracking_tables(&pool, &config.database_type).await?;
        let migrations = pool.get_applied_migrations().await?;
        Ok(migrations
            .iter()
            .any(|m| m.status == MigrationStatus::Success.as_str()))
    }

    /// Marks every migration in `baseline.source` whose version is <= `baseline.version`
    /// as already applied, with no SQL executed - the schema those migrations describe
    /// was already generated wholesale from `schema.xml` by the `install` call this
    /// follows. Mirrors Flyway's `baseline`: it records history, it doesn't run scripts.
    async fn record_baseline(
        pool: &AnyPool,
        baseline: Baseline<'_>,
        tool_version: &str,
    ) -> Result<(), SchemaInstallerError> {
        if baseline.version == crate::migration::RESERVED_INSTALL_VERSION {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "Baseline version '{}' is reserved for the install command's own tracking row and cannot be used as a baseline version",
                crate::migration::RESERVED_INSTALL_VERSION
            )));
        }

        let mut migrations = baseline.source.migrations()?;
        migrations.sort_by(|a, b| compare_versions(&a.version, &b.version));
        migrations.retain(|m| {
            compare_versions(&m.version, baseline.version) != std::cmp::Ordering::Greater
        });

        for migration in &migrations {
            let checksum = compute_checksum(&migration.sql);
            pool.insert_migration(
                &migration.version,
                &migration.script_path,
                &migration.description,
                &checksum,
                0,
                MigrationStatus::Success.as_str(),
                tool_version,
            )
            .await?;
        }

        println!(
            "Baselined {} migration(s) up to version {}.",
            migrations.len(),
            baseline.version
        );

        Ok(())
    }

    pub async fn is_installed(
        config: &SchemaInstallerConfig,
    ) -> Result<bool, SchemaInstallerError> {
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;
        // Create-if-missing rather than sniffing the "table doesn't exist" error text, which
        // is dialect-specific (SQL Server says `Invalid object name`) and so made a fresh
        // database return `Err` instead of "not installed" (M22). Every other command already
        // bootstraps the table this way.
        Self::ensure_tracking_tables(&pool, &config.database_type).await?;
        Self::check_if_installed(&pool).await
    }

    pub async fn get_installed_version(
        config: &SchemaInstallerConfig,
    ) -> Result<Option<String>, SchemaInstallerError> {
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;
        Self::ensure_tracking_tables(&pool, &config.database_type).await?;
        let migrations = pool.get_applied_migrations().await?;
        let latest = migrations
            .iter()
            .filter(|m| m.status == MigrationStatus::Success.as_str())
            .max_by(|a, b| crate::migration::compare_versions(&a.version, &b.version));
        Ok(latest.map(|m| m.version.clone()))
    }

    async fn check_if_installed(pool: &AnyPool) -> Result<bool, SchemaInstallerError> {
        // Scoped to the reserved `install` tracking row only (H14): checking for *any*
        // successful row conflated "the XML install ran" with "a Flyway-style migration
        // ran", so running `migrate` first made a later `install` report "already
        // installed" and exit 0 without creating a single table. Callers must have run
        // `ensure_tracking_tables` first.
        let migrations = pool.get_applied_migrations().await?;
        Ok(migrations.iter().any(|m| {
            m.status == MigrationStatus::Success.as_str()
                && m.version == crate::migration::RESERVED_INSTALL_VERSION
        }))
    }

    async fn ensure_tracking_tables(
        pool: &AnyPool,
        database_type: &GeneratorType,
    ) -> Result<(), SchemaInstallerError> {
        pool.ensure_migration_table(database_type).await?;
        Ok(())
    }
}

/// A cloneable in-memory `Write` sink: the generator takes ownership of one clone (boxed
/// into its `PrintWriter`) while the installer keeps another to read the SQL back out.
#[derive(Clone, Default)]
struct SharedBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl SharedBuffer {
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

impl std::io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_buffer_collects_writes_from_every_clone() {
        use std::io::Write;
        let buffer = SharedBuffer::default();
        let mut writer = buffer.clone();
        writer.write_all(b"create table t (").unwrap();
        writer.write_all(b"id int);").unwrap();
        assert_eq!(buffer.take(), b"create table t (id int);");
        assert!(buffer.take().is_empty());
    }
}
