use schema_sql_generator::common::generator_type::GeneratorType;
use std::collections::HashSet;
use std::time::Instant;

use crate::config::SchemaInstallerConfig;
use crate::connection::AnyPool;
use crate::error::SchemaInstallerError;
use crate::migration::{compare_versions, compute_checksum, MigrationSource};

pub struct Migrator;

impl Migrator {
    pub async fn migrate(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        pool.ensure_migration_table(&config.database_type)
            .await?;

        let applied = pool.get_applied_migrations().await?;
        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == "success")
            .map(|m| m.version.clone())
            .collect();

        let source_migrations = source.migrations()?;

        for applied_migration in &applied {
            if applied_migration.status == "success" {
                if let Some(source_migration) = source_migrations
                    .iter()
                    .find(|m| m.version == applied_migration.version)
                {
                    let checksum = compute_checksum(&source_migration.sql);
                    if checksum != applied_migration.checksum {
                        return Err(SchemaInstallerError::ChecksumMismatch {
                            version: applied_migration.version.clone(),
                            expected: applied_migration.checksum.clone(),
                            found: checksum,
                        });
                    }
                }
            }
        }

        let mut migrations = source_migrations;
        migrations.retain(|m| !applied_versions.contains(&m.version));

        if migrations.is_empty() {
            println!("No pending migrations to apply");
            return Ok(());
        }

        let tool_version = env!("CARGO_PKG_VERSION");

        for migration in migrations {
            let checksum = compute_checksum(&migration.sql);
            let migration_id = pool
                .insert_migration(
                    &migration.version,
                    &migration.script_path,
                    &checksum,
                    0,
                    "pending",
                    tool_version,
                )
                .await?;

            let start = Instant::now();
            match execute_migration(&pool, &config.database_type, &migration.sql).await {
                Ok(_) => {
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    pool.update_migration_status(migration_id, "success", elapsed_ms)
                        .await?;
                    println!(
                        "Applied migration: {} - {}",
                        migration.version, migration.description
                    );
                }
                Err(e) => {
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    pool.update_migration_status(migration_id, "failed", elapsed_ms)
                        .await?;
                    return Err(SchemaInstallerError::MigrationFailed {
                        version: migration.version,
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    pub async fn info(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        if let Err(_) = pool.ensure_migration_table(&config.database_type).await {
        }

        let applied = pool.get_applied_migrations().await.unwrap_or_default();
        let source_migrations = source.migrations()?;

        if applied.is_empty() && source_migrations.is_empty() {
            println!("No migrations found");
            return Ok(());
        }

        println!(
            "{:<10} {:<30} {:<10} {:<30} {:<15}",
            "Version", "Description", "Status", "Installed At", "Execution (ms)"
        );
        println!("{}", "-".repeat(95));

        let mut all_versions: Vec<String> = applied.iter().map(|m| m.version.clone()).collect();
        for migration in &source_migrations {
            if !all_versions.contains(&migration.version) {
                all_versions.push(migration.version.clone());
            }
        }

        all_versions.sort_by(|a, b| compare_versions(a, b));

        for version in all_versions {
            if let Some(applied_mig) = applied.iter().find(|m| m.version == version) {
                println!(
                    "{:<10} {:<30} {:<10} {:<30} {:<15}",
                    applied_mig.version,
                    applied_mig.script_path.split('/').last().unwrap_or(""),
                    applied_mig.status,
                    applied_mig.installed_at,
                    applied_mig.execution_time_ms
                );
            } else if let Some(source_mig) = source_migrations.iter().find(|m| m.version == version) {
                println!(
                    "{:<10} {:<30} {:<10} {:<30} {:<15}",
                    version,
                    source_mig.description,
                    "Pending",
                    "-",
                    "-"
                );
            }
        }

        Ok(())
    }

    pub async fn validate(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        pool.ensure_migration_table(&config.database_type)
            .await?;

        let applied = pool.get_applied_migrations().await?;
        let source_migrations = source.migrations()?;

        let mut mismatches = Vec::new();

        for applied_migration in applied {
            if applied_migration.status != "success" {
                continue;
            }

            if let Some(source_migration) = source_migrations
                .iter()
                .find(|m| m.version == applied_migration.version)
            {
                let checksum = compute_checksum(&source_migration.sql);
                if checksum != applied_migration.checksum {
                    mismatches.push((
                        applied_migration.version.clone(),
                        applied_migration.checksum.clone(),
                        checksum,
                    ));
                }
            }
        }

        if !mismatches.is_empty() {
            for (version, expected, found) in mismatches {
                eprintln!(
                    "Checksum mismatch for version {}: expected {}, found {}",
                    version, expected, found
                );
            }
            return Err(SchemaInstallerError::ChecksumMismatch {
                version: "unknown".to_string(),
                expected: "see above".to_string(),
                found: "see above".to_string(),
            });
        }

        println!("All migrations validated successfully");
        Ok(())
    }

    pub async fn has_pending_migrations(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<bool, SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        if pool.ensure_migration_table(&config.database_type).await.is_err() {
            return Ok(true);
        }

        let applied = pool.get_applied_migrations().await.unwrap_or_default();
        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == "success")
            .map(|m| m.version.clone())
            .collect();

        let source_migrations = source.migrations()?;
        let pending = source_migrations
            .iter()
            .any(|m| !applied_versions.contains(&m.version));

        Ok(pending)
    }

    pub async fn repair(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        pool.delete_failed_migrations().await?;
        println!("Deleted failed migrations");

        let applied = pool.get_applied_migrations().await?;
        let source_migrations = source.migrations()?;

        for applied_migration in applied {
            if applied_migration.status != "success" {
                continue;
            }

            if let Some(source_migration) = source_migrations
                .iter()
                .find(|m| m.version == applied_migration.version)
            {
                let checksum = compute_checksum(&source_migration.sql);
                if checksum != applied_migration.checksum {
                    pool.update_migration_checksum(applied_migration.id, &checksum)
                        .await?;
                    println!(
                        "Updated checksum for migration: {}",
                        applied_migration.version
                    );
                }
            }
        }

        Ok(())
    }
}

async fn execute_migration(
    pool: &AnyPool,
    database_type: &GeneratorType,
    sql: &str,
) -> Result<(), SchemaInstallerError> {
    for statement in crate::sql_split::split_sql_statements(sql, database_type) {
        pool.execute_sql(&statement).await?;
    }

    Ok(())
}
