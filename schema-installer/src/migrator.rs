use schema_sql_generator::common::generator_type::GeneratorType;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::SchemaInstallerConfig;
use crate::connection::AnyPool;
use crate::error::SchemaInstallerError;
use crate::migration::{Migration, MigrationSource, RESERVED_INSTALL_VERSION, compare_versions, compute_checksum};

/// How often to re-check a colliding migration's status while waiting for whichever
/// process is currently applying it to finish.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(500);
/// How long to wait for a colliding migration to resolve before giving up. Mirrors
/// Flyway's bounded lock-retry behavior rather than waiting forever for a process that
/// may have crashed while holding the row.
const LOCK_MAX_WAIT: Duration = Duration::from_secs(60);

/// How old a "pending" tracking row must be before `repair` treats it as abandoned by a
/// crashed process rather than a migration another instance might still legitimately be
/// running. Deliberately much larger than `LOCK_MAX_WAIT`: by the time an operator runs
/// `repair` by hand, any real migration that was ever going to finish already has, so
/// this mainly guards against `repair` racing a process that is still within its normal
/// (possibly long-running) execution window.
const STALE_PENDING_THRESHOLD: Duration = Duration::from_secs(600);


/// Outcome of claiming the "pending" slot for a migration version.
enum Slot {
    /// This process won the race and should execute the migration itself.
    Owned(i64),
    /// Another process already applied this version successfully while we were
    /// waiting; nothing left for this process to do for it.
    AlreadyApplied,
}

/// Claims the `"pending"` tracking row for `migration`, the single point of mutual
/// exclusion between concurrent instances (enforced by the `UNIQUE (version)`
/// constraint on `schema_migration`). If another process already holds it, this waits
/// and watches that row rather than failing outright — matching Flyway's behavior of
/// blocking a racing instance until the winner's run resolves, then either skipping
/// (if it succeeded) or surfacing the failure (if it didn't).
async fn wait_for_slot(
    pool: &AnyPool,
    migration: &Migration,
    checksum: &str,
    tool_version: &str,
) -> Result<Slot, SchemaInstallerError> {
    match insert_pending(pool, migration, checksum, tool_version).await {
        Ok(id) => Ok(Slot::Owned(id)),
        Err(SchemaInstallerError::ConcurrentMigrationDetected(version)) => {
            // The colliding row may belong to a process that already crashed while
            // holding it rather than one still legitimately running. Purge anything
            // stale enough to qualify (same threshold `repair` uses) and try again
            // immediately, so an abandoned lock self-heals right away instead of
            // wedging every future `migrate` run for `LOCK_MAX_WAIT` at a time until
            // an operator runs `repair` by hand.
            pool.delete_stale_pending_migrations(STALE_PENDING_THRESHOLD.as_secs() as i64)
                .await?;

            match insert_pending(pool, migration, checksum, tool_version).await {
                Ok(id) => return Ok(Slot::Owned(id)),
                Err(SchemaInstallerError::ConcurrentMigrationDetected(_)) => {} // not stale; fall through to poll
                Err(e) => return Err(e),
            }

            let deadline = Instant::now() + LOCK_MAX_WAIT;
            loop {
                let applied = pool.get_applied_migrations().await?;
                if let Some(existing) = applied.iter().find(|m| m.version == version) {
                    match existing.status.as_str() {
                        "success" => return Ok(Slot::AlreadyApplied),
                        "failed" => {
                            return Err(SchemaInstallerError::MigrationFailed {
                                version: version.clone(),
                                error: "a concurrent process already attempted this migration and it failed; run `repair` before retrying".to_string(),
                            });
                        }
                        _ => {} // still "pending" elsewhere; keep waiting below
                    }
                }

                if Instant::now() >= deadline {
                    return Err(SchemaInstallerError::LockTimeout(version));
                }
                tokio::time::sleep(LOCK_POLL_INTERVAL).await;
            }
        }
        Err(e) => Err(e),
    }
}

async fn insert_pending(
    pool: &AnyPool,
    migration: &Migration,
    checksum: &str,
    tool_version: &str,
) -> Result<i64, SchemaInstallerError> {
    pool.insert_migration(&migration.version, &migration.script_path, checksum, 0, "pending", tool_version)
        .await
}

pub struct Migrator;

impl Migrator {
    pub async fn migrate(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        Self::migrate_to_target(config, source, None).await
    }

    /// Like `migrate`, but when `target` is given, only applies migrations with a
    /// version <= `target` even if later ones exist in `source` - Flyway's `-target`.
    /// Lets a rollout intentionally stop at a known-good version instead of always
    /// applying everything pending.
    pub async fn migrate_to_target(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
        target: Option<&str>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        pool.ensure_migration_table(&config.database_type)
            .await?;

        let migrations = Self::resolve_pending_migrations(&pool, source, target).await?;

        if migrations.is_empty() {
            println!("No pending migrations to apply");
            return Ok(());
        }

        let tool_version = env!("CARGO_PKG_VERSION");

        for migration in migrations {
            let checksum = compute_checksum(&migration.sql);
            let migration_id = match wait_for_slot(&pool, &migration, &checksum, tool_version).await? {
                Slot::Owned(id) => id,
                Slot::AlreadyApplied => {
                    println!(
                        "Migration {} - {} was already applied by another process; skipping",
                        migration.version, migration.description
                    );
                    continue;
                }
            };

            let start = Instant::now();
            match execute_migration(&pool, &config.database_type, migration_id, &migration.sql).await {
                Ok(_) => {
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

    /// Like `migrate_to_target`, but instead of executing anything, writes the SQL of
    /// every migration that *would* be applied - in order, unexecuted - to
    /// `output_path` (Flyway's `-dryRun`). Nothing is recorded in `schema_migration`
    /// and no statement is ever sent to the database's execution path, so this is safe
    /// to run against a production connection for review before a real `migrate`.
    pub async fn dry_run(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
        target: Option<&str>,
        output_path: &Path,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        pool.ensure_migration_table(&config.database_type)
            .await?;

        let migrations = Self::resolve_pending_migrations(&pool, source, target).await?;

        let mut output = String::new();
        if migrations.is_empty() {
            writeln!(output, "-- No pending migrations.").ok();
        } else {
            for migration in &migrations {
                writeln!(
                    output,
                    "-- Migration {} - {} ({})",
                    migration.version, migration.description, migration.script_path
                )
                .ok();
                output.push_str(migration.sql.trim_end());
                output.push_str("\n\n");
            }
        }

        std::fs::write(output_path, output).map_err(SchemaInstallerError::Io)?;

        println!(
            "Wrote dry-run SQL for {} pending migration(s) to {}",
            migrations.len(),
            output_path.display()
        );

        Ok(())
    }

    /// Shared by `migrate_to_target` and `dry_run`: resolves the set of migrations that
    /// would be applied by a real `migrate` run - already-applied migrations filtered
    /// out, checksums validated against tracked history, `target` applied, and the
    /// out-of-order check enforced - without executing any of them.
    async fn resolve_pending_migrations(
        pool: &AnyPool,
        source: Box<dyn MigrationSource>,
        target: Option<&str>,
    ) -> Result<Vec<Migration>, SchemaInstallerError> {
        let applied = pool.get_applied_migrations().await?;
        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == "success")
            .map(|m| m.version.clone())
            .collect();

        // Sort here rather than trusting each `MigrationSource` impl to return migrations
        // in version order: `DirectoryMigrationSource` happens to sort internally, but
        // `EmbeddedMigrationSource` (and any other future source) just returns whatever
        // order it was given, which would otherwise let migrations apply out of order.
        let mut source_migrations = source.migrations()?;
        source_migrations.sort_by(|a, b| compare_versions(&a.version, &b.version));

        for applied_migration in &applied {
            if applied_migration.status != "success" {
                continue;
            }

            match source_migrations
                .iter()
                .find(|m| m.version == applied_migration.version)
            {
                Some(source_migration) => {
                    let checksum = compute_checksum(&source_migration.sql);
                    if checksum != applied_migration.checksum {
                        return Err(SchemaInstallerError::ChecksumMismatch {
                            version: applied_migration.version.clone(),
                            expected: applied_migration.checksum.clone(),
                            found: checksum,
                        });
                    }
                }
                // The legacy XML `install` command's reserved tracking row never
                // corresponds to a real migration file; see RESERVED_INSTALL_VERSION.
                None if applied_migration.version == RESERVED_INSTALL_VERSION => {}
                None => {
                    // Same drift `validate` catches: an applied migration whose file was
                    // since deleted or renamed. Caught here too so it blocks `migrate`
                    // automatically, matching Flyway's validateOnMigrate default instead
                    // of only surfacing when someone thinks to run `validate` by hand.
                    return Err(SchemaInstallerError::MissingMigrationSource {
                        version: applied_migration.version.clone(),
                        script_path: applied_migration.script_path.clone(),
                    });
                }
            }
        }

        let mut migrations = source_migrations;
        migrations.retain(|m| !applied_versions.contains(&m.version));

        if let Some(target) = target {
            migrations.retain(|m| compare_versions(&m.version, target) != std::cmp::Ordering::Greater);
        }

        if migrations.is_empty() {
            return Ok(migrations);
        }

        // Refuse to apply a migration older than the highest version already applied,
        // the same way Flyway errors by default (`outOfOrder=false`) rather than
        // silently running it against a schema state it was never designed for - e.g. a
        // branch's migration merged after a later-numbered one already dropped a table
        // it depends on.
        if let Some(highest_applied) = applied_versions.iter().max_by(|a, b| compare_versions(a, b))
            && let Some(out_of_order) = migrations
                .iter()
                .find(|m| compare_versions(&m.version, highest_applied) == std::cmp::Ordering::Less)
        {
            return Err(SchemaInstallerError::OutOfOrderMigration {
                version: out_of_order.version.clone(),
                description: out_of_order.description.clone(),
                highest_applied: highest_applied.clone(),
            });
        }

        Ok(migrations)
    }

    pub async fn info(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(&config.database_type, &config.connection_string).await?;

        pool.ensure_migration_table(&config.database_type).await?;

        let applied = pool.get_applied_migrations().await?;
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

        // Same condition `migrate` rejects with `OutOfOrderMigration` and `validate` reports
        // as an issue: a source migration older than the highest successfully-applied one
        // that was never applied. `info` can't apply it either, so it's mislabeled as
        // ordinary "Pending" otherwise - matching Flyway's own "Ignored" state in `info`.
        let highest_applied = applied
            .iter()
            .filter(|m| m.status == "success")
            .map(|m| m.version.clone())
            .max_by(|a, b| compare_versions(a, b));

        for version in all_versions {
            if let Some(applied_mig) = applied.iter().find(|m| m.version == version) {
                println!(
                    "{:<10} {:<30} {:<10} {:<30} {:<15}",
                    applied_mig.version,
                    applied_mig.script_path.split('/').next_back().unwrap_or(""),
                    applied_mig.status,
                    applied_mig.installed_at,
                    applied_mig.execution_time_ms
                );
            } else if let Some(source_mig) = source_migrations.iter().find(|m| m.version == version) {
                let status = match &highest_applied {
                    Some(highest)
                        if compare_versions(&version, highest) == std::cmp::Ordering::Less =>
                    {
                        "Ignored"
                    }
                    _ => "Pending",
                };
                println!(
                    "{:<10} {:<30} {:<10} {:<30} {:<15}",
                    version, source_mig.description, status, "-", "-"
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

        let mut issue_count = 0usize;

        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == "success")
            .map(|m| m.version.clone())
            .collect();

        for applied_migration in &applied {
            if applied_migration.status != "success" {
                // `migrate` never treats these as harmless: a "failed" row makes the next
                // `migrate` attempt collide and abort ("run `repair` before retrying"), and
                // a "pending" row does the same after `LOCK_MAX_WAIT`. Reporting them here
                // instead of silently skipping is what closes BUGS_AND_GAPS H10.
                issue_count += 1;
                eprintln!(
                    "Migration {} is in a non-successful state ({}); run `repair` before retrying",
                    applied_migration.version, applied_migration.status
                );
                continue;
            }

            match source_migrations
                .iter()
                .find(|m| m.version == applied_migration.version)
            {
                Some(source_migration) => {
                    let checksum = compute_checksum(&source_migration.sql);
                    if checksum != applied_migration.checksum {
                        issue_count += 1;
                        eprintln!(
                            "Checksum mismatch for version {}: expected {}, found {}",
                            applied_migration.version, applied_migration.checksum, checksum
                        );
                    }
                }
                // The reserved `install_version` used by the legacy XML `install` command
                // (see installer.rs) never corresponds to a real migration file, so it's
                // exempt from the missing-source check below.
                None if applied_migration.version == RESERVED_INSTALL_VERSION => {}
                None => {
                    issue_count += 1;
                    eprintln!(
                        "{}",
                        SchemaInstallerError::MissingMigrationSource {
                            version: applied_migration.version.clone(),
                            script_path: applied_migration.script_path.clone(),
                        }
                    );
                }
            }
        }

        // Mirror `migrate`'s out-of-order check: a resolved migration older than the
        // highest successfully-applied version that was never applied at all. `migrate`
        // rejects this with `OutOfOrderMigration` as soon as it tries to run it; Flyway
        // reports the same state as "Detected resolved migration not applied to database".
        // Without this, `validate` never looks at unapplied source migrations at all.
        if let Some(highest_applied) = applied_versions.iter().max_by(|a, b| compare_versions(a, b)) {
            for source_migration in &source_migrations {
                if !applied_versions.contains(&source_migration.version)
                    && compare_versions(&source_migration.version, highest_applied) == std::cmp::Ordering::Less
                {
                    issue_count += 1;
                    eprintln!(
                        "Detected resolved migration not applied to database: {} - {}",
                        source_migration.version, source_migration.description
                    );
                }
            }
        }

        if issue_count > 0 {
            return Err(SchemaInstallerError::ValidationFailed(format!(
                "{} validation issue(s) found; see above for details",
                issue_count
            )));
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

        pool.ensure_migration_table(&config.database_type)
            .await?;

        pool.delete_failed_migrations().await?;
        println!("Deleted failed migrations");

        pool.delete_stale_pending_migrations(STALE_PENDING_THRESHOLD.as_secs() as i64)
            .await?;
        println!(
            "Deleted pending migrations stuck for over {} seconds (likely abandoned by a crashed process)",
            STALE_PENDING_THRESHOLD.as_secs()
        );

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
    migration_id: i64,
    sql: &str,
) -> Result<i64, SchemaInstallerError> {
    let statements = crate::sql_split::split_sql_statements(sql, database_type);
    // All statements in a migration file, plus the tracking row's "success" update,
    // commit or roll back together as one transaction - see `execute_migration_transactional`
    // for why (BUGS_AND_GAPS H9).
    pool.execute_migration_transactional(&statements, migration_id).await
}
