use schema_sql_generator::common::generator_type::GeneratorType;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::SchemaInstallerConfig;
use crate::connection::AnyPool;
use crate::error::SchemaInstallerError;
use crate::migration::{
    AppliedRepeatableMigration, Migration, MigrationSource, MigrationStatus,
    RESERVED_INSTALL_VERSION, RepeatableMigration, compare_versions, compute_checksum,
};

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
                    match existing.status.parse::<MigrationStatus>() {
                        Ok(MigrationStatus::Success) => return Ok(Slot::AlreadyApplied),
                        Ok(MigrationStatus::Failed) => {
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
    pool.insert_migration(
        &migration.version,
        &migration.script_path,
        &migration.description,
        checksum,
        0,
        MigrationStatus::Pending.as_str(),
        tool_version,
    )
    .await
}

/// The version label used in errors for a repeatable, e.g. `R__refresh_views`.
fn repeatable_label(repeatable: &RepeatableMigration) -> String {
    format!("R__{}", repeatable.name)
}

/// Claims the right to run `repeatable`, the repeatable counterpart of `wait_for_slot`.
/// Mutual exclusion between concurrent instances comes from `UNIQUE (name)` for a first run
/// and from the checksum-conditional `repend_repeatable` for a re-run of a changed script.
/// Each pass re-reads the row, so a losing instance simply observes the winner's outcome:
/// `success` with our checksum means the work is already done, `failed` surfaces the failure,
/// and `pending` is waited out (after one stale-row cleanup, as in `wait_for_slot`).
async fn claim_repeatable(
    pool: &AnyPool,
    repeatable: &RepeatableMigration,
    checksum: &str,
    tool_version: &str,
) -> Result<Slot, SchemaInstallerError> {
    let deadline = Instant::now() + LOCK_MAX_WAIT;
    let mut purged_stale = false;

    loop {
        let applied = pool.get_applied_repeatables().await?;
        let Some(existing) = applied.iter().find(|r| r.name == repeatable.name) else {
            match pool
                .insert_repeatable(
                    &repeatable.name,
                    &repeatable.script_path,
                    checksum,
                    0,
                    MigrationStatus::Pending.as_str(),
                    tool_version,
                )
                .await
            {
                Ok(id) => return Ok(Slot::Owned(id)),
                // Someone inserted first; the next pass reads their row.
                Err(SchemaInstallerError::ConcurrentMigrationDetected(_)) => continue,
                Err(e) => return Err(e),
            }
        };

        match existing.status.parse::<MigrationStatus>() {
            Ok(MigrationStatus::Success) if existing.checksum == checksum => {
                return Ok(Slot::AlreadyApplied);
            }
            Ok(MigrationStatus::Success) => {
                if pool
                    .repend_repeatable(
                        existing.id,
                        &existing.checksum,
                        checksum,
                        &repeatable.script_path,
                        tool_version,
                    )
                    .await?
                {
                    return Ok(Slot::Owned(existing.id));
                }
                // Lost the claim to another instance; re-read what it did.
            }
            Ok(MigrationStatus::Failed) => {
                return Err(SchemaInstallerError::MigrationFailed {
                    version: repeatable_label(repeatable),
                    error: "a previous attempt at this repeatable migration failed; run `repair` before retrying".to_string(),
                });
            }
            _ => {
                if !purged_stale {
                    purged_stale = true;
                    let purged = pool
                        .delete_stale_pending_repeatables(STALE_PENDING_THRESHOLD.as_secs() as i64)
                        .await?;
                    if purged > 0 {
                        continue;
                    }
                }
                if Instant::now() >= deadline {
                    return Err(SchemaInstallerError::LockTimeout(repeatable_label(
                        repeatable,
                    )));
                }
                tokio::time::sleep(LOCK_POLL_INTERVAL).await;
            }
        }
    }
}

/// The description `info` shows for an applied row. Rows written before the `description`
/// column existed (M14) have it empty, so fall back to deriving it from the recorded filename
/// the way `migrate` would, and failing that to the filename itself.
fn applied_description(applied: &crate::migration::AppliedMigration) -> String {
    if !applied.description.is_empty() {
        return applied.description.clone();
    }
    let filename = applied.script_path.rsplit(['/', '\\']).next().unwrap_or("");
    match crate::migration::parse_migration_filename(filename) {
        Ok(Some((_, description))) if !description.is_empty() => description,
        _ => filename.to_string(),
    }
}

/// Whether `repeatable` needs to run: it has never succeeded, or its script changed since the
/// last successful run. A `failed`/`pending` row counts, so it is surfaced rather than skipped.
fn repeatable_is_pending(
    applied: &[AppliedRepeatableMigration],
    repeatable: &RepeatableMigration,
) -> bool {
    match applied.iter().find(|a| a.name == repeatable.name) {
        Some(a) => {
            a.status != MigrationStatus::Success.as_str()
                || a.checksum != compute_checksum(&repeatable.sql)
        }
        None => true,
    }
}

/// The `Status` column `info` shows for a repeatable. Flyway's `Outdated` marks one that was
/// applied but whose script has since changed (so the next `migrate` re-runs it).
fn repeatable_info_status(
    applied: Option<&AppliedRepeatableMigration>,
    source: Option<&RepeatableMigration>,
) -> String {
    match (applied, source) {
        (Some(a), Some(r))
            if a.status == MigrationStatus::Success.as_str()
                && a.checksum != compute_checksum(&r.sql) =>
        {
            "Outdated".to_string()
        }
        (Some(a), _) => a.status.clone(),
        (None, _) => "Pending".to_string(),
    }
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
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        pool.ensure_migration_table(&config.database_type).await?;

        let migrations = Self::resolve_pending_migrations(&pool, source.as_ref(), target).await?;
        let repeatables = Self::resolve_pending_repeatables(&pool, source.as_ref(), target).await?;

        if migrations.is_empty() && repeatables.is_empty() {
            println!("No pending migrations to apply");
            return Ok(());
        }

        validate_migration_scripts(&migrations, &repeatables, &config.database_type)?;

        let tool_version = env!("CARGO_PKG_VERSION");

        for migration in migrations {
            let checksum = compute_checksum(&migration.sql);
            let migration_id =
                match wait_for_slot(&pool, &migration, &checksum, tool_version).await? {
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
            match execute_migration(&pool, &config.database_type, migration_id, &migration).await {
                Ok(_) => {
                    println!(
                        "Applied migration: {} - {}",
                        migration.version, migration.description
                    );
                }
                Err(e) => {
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    pool.update_migration_status(
                        migration_id,
                        MigrationStatus::Failed.as_str(),
                        elapsed_ms,
                    )
                    .await?;
                    return Err(SchemaInstallerError::MigrationFailed {
                        version: migration.version,
                        error: e.to_string(),
                    });
                }
            }
        }

        // Repeatables run last, once every versioned migration they may depend on is in.
        for repeatable in repeatables {
            let checksum = compute_checksum(&repeatable.sql);
            let repeatable_id = match claim_repeatable(&pool, &repeatable, &checksum, tool_version)
                .await?
            {
                Slot::Owned(id) => id,
                Slot::AlreadyApplied => {
                    println!(
                        "Repeatable migration {} was already applied by another process; skipping",
                        repeatable.description()
                    );
                    continue;
                }
            };

            let start = Instant::now();
            match execute_repeatable(&pool, &config.database_type, repeatable_id, &repeatable).await
            {
                Ok(_) => println!("Applied repeatable migration: {}", repeatable.description()),
                Err(e) => {
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    pool.update_repeatable_status(
                        repeatable_id,
                        MigrationStatus::Failed.as_str(),
                        elapsed_ms,
                    )
                    .await?;
                    return Err(SchemaInstallerError::MigrationFailed {
                        version: repeatable_label(&repeatable),
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
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        pool.ensure_migration_table(&config.database_type).await?;

        let migrations = Self::resolve_pending_migrations(&pool, source.as_ref(), target).await?;
        let repeatables = Self::resolve_pending_repeatables(&pool, source.as_ref(), target).await?;

        validate_migration_scripts(&migrations, &repeatables, &config.database_type)?;

        let mut output = String::new();
        if migrations.is_empty() && repeatables.is_empty() {
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
            for repeatable in &repeatables {
                writeln!(
                    output,
                    "-- Repeatable migration {} ({})",
                    repeatable.description(),
                    repeatable.script_path
                )
                .ok();
                output.push_str(repeatable.sql.trim_end());
                output.push_str("\n\n");
            }
        }

        std::fs::write(output_path, output).map_err(SchemaInstallerError::Io)?;

        println!(
            "Wrote dry-run SQL for {} pending migration(s) to {}",
            migrations.len() + repeatables.len(),
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
        source: &dyn MigrationSource,
        target: Option<&str>,
    ) -> Result<Vec<Migration>, SchemaInstallerError> {
        let applied = pool.get_applied_migrations().await?;
        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == MigrationStatus::Success.as_str())
            .map(|m| m.version.clone())
            .collect();

        // Sort here rather than trusting each `MigrationSource` impl to return migrations
        // in version order: `DirectoryMigrationSource` happens to sort internally, but
        // `EmbeddedMigrationSource` (and any other future source) just returns whatever
        // order it was given, which would otherwise let migrations apply out of order.
        let mut source_migrations = source.migrations()?;
        source_migrations.sort_by(|a, b| compare_versions(&a.version, &b.version));

        for applied_migration in &applied {
            if applied_migration.status != MigrationStatus::Success.as_str() {
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
            migrations
                .retain(|m| compare_versions(&m.version, target) != std::cmp::Ordering::Greater);
        }

        if migrations.is_empty() {
            return Ok(migrations);
        }

        // Refuse to apply a migration older than the highest version already applied,
        // the same way Flyway errors by default (`outOfOrder=false`) rather than
        // silently running it against a schema state it was never designed for - e.g. a
        // branch's migration merged after a later-numbered one already dropped a table
        // it depends on.
        if let Some(highest_applied) = applied_versions
            .iter()
            .max_by(|a, b| compare_versions(a, b))
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

    /// The repeatables a real `migrate` would run: new ones, and ones whose script changed
    /// since their last successful run. A row that is `failed` or `pending` is also returned
    /// so `claim_repeatable` surfaces it (blocking until `repair`) instead of skipping it
    /// silently. `target` skips repeatables entirely: a rollout deliberately stopped at a
    /// version should not run scripts written against a newer schema.
    async fn resolve_pending_repeatables(
        pool: &AnyPool,
        source: &dyn MigrationSource,
        target: Option<&str>,
    ) -> Result<Vec<RepeatableMigration>, SchemaInstallerError> {
        if target.is_some() {
            return Ok(Vec::new());
        }

        let source_repeatables = source.repeatable_migrations()?;
        let applied = pool.get_applied_repeatables().await?;

        for applied_repeatable in &applied {
            if applied_repeatable.status == MigrationStatus::Success.as_str()
                && !source_repeatables
                    .iter()
                    .any(|r| r.name == applied_repeatable.name)
            {
                // Same drift `resolve_pending_migrations` blocks on for versioned ones.
                return Err(SchemaInstallerError::MissingMigrationSource {
                    version: format!("R__{}", applied_repeatable.name),
                    script_path: applied_repeatable.script_path.clone(),
                });
            }
        }

        Ok(source_repeatables
            .into_iter()
            .filter(|repeatable| repeatable_is_pending(&applied, repeatable))
            .collect())
    }

    pub async fn info(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        pool.ensure_migration_table(&config.database_type).await?;

        let applied = pool.get_applied_migrations().await?;
        let source_migrations = source.migrations()?;
        let applied_repeatables = pool.get_applied_repeatables().await?;
        let source_repeatables = source.repeatable_migrations()?;

        if applied.is_empty()
            && source_migrations.is_empty()
            && applied_repeatables.is_empty()
            && source_repeatables.is_empty()
        {
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
            .filter(|m| m.status == MigrationStatus::Success.as_str())
            .map(|m| m.version.clone())
            .max_by(|a, b| compare_versions(a, b));

        for version in all_versions {
            if let Some(applied_mig) = applied.iter().find(|m| m.version == version) {
                println!(
                    "{:<10} {:<30} {:<10} {:<30} {:<15}",
                    applied_mig.version,
                    applied_description(applied_mig),
                    applied_mig.status,
                    applied_mig.installed_at,
                    applied_mig.execution_time_ms
                );
            } else if let Some(source_mig) = source_migrations.iter().find(|m| m.version == version)
            {
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

        // Repeatables follow the versioned rows, in name order, as they run.
        let mut repeatable_names: Vec<&str> = applied_repeatables
            .iter()
            .map(|r| r.name.as_str())
            .chain(source_repeatables.iter().map(|r| r.name.as_str()))
            .collect();
        repeatable_names.sort_unstable();
        repeatable_names.dedup();

        for name in repeatable_names {
            let applied_repeatable = applied_repeatables.iter().find(|r| r.name == name);
            let source_repeatable = source_repeatables.iter().find(|r| r.name == name);
            let status = repeatable_info_status(applied_repeatable, source_repeatable);
            println!(
                "{:<10} {:<30} {:<10} {:<30} {:<15}",
                "R",
                name.replace('_', " "),
                status,
                applied_repeatable.map_or("-", |r| r.installed_at.as_str()),
                applied_repeatable.map_or("-".to_string(), |r| r.execution_time_ms.to_string()),
            );
        }

        Ok(())
    }

    pub async fn validate(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        pool.ensure_migration_table(&config.database_type).await?;

        let applied = pool.get_applied_migrations().await?;
        let source_migrations = source.migrations()?;

        let mut issue_count = 0usize;

        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == MigrationStatus::Success.as_str())
            .map(|m| m.version.clone())
            .collect();

        for applied_migration in &applied {
            if applied_migration.status != MigrationStatus::Success.as_str() {
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
        if let Some(highest_applied) = applied_versions
            .iter()
            .max_by(|a, b| compare_versions(a, b))
        {
            for source_migration in &source_migrations {
                if !applied_versions.contains(&source_migration.version)
                    && compare_versions(&source_migration.version, highest_applied)
                        == std::cmp::Ordering::Less
                {
                    issue_count += 1;
                    eprintln!(
                        "Detected resolved migration not applied to database: {} - {}",
                        source_migration.version, source_migration.description
                    );
                }
            }
        }

        let applied_repeatables = pool.get_applied_repeatables().await?;
        let source_repeatables = source.repeatable_migrations()?;

        for applied_repeatable in &applied_repeatables {
            let label = format!("R__{}", applied_repeatable.name);
            if applied_repeatable.status != MigrationStatus::Success.as_str() {
                issue_count += 1;
                eprintln!(
                    "Repeatable migration {} is in a non-successful state ({}); run `repair` before retrying",
                    label, applied_repeatable.status
                );
                continue;
            }
            match source_repeatables
                .iter()
                .find(|r| r.name == applied_repeatable.name)
            {
                // A changed script is not an error: it is simply pending, and the next
                // `migrate` re-runs it.
                Some(source_repeatable) => {
                    if compute_checksum(&source_repeatable.sql) != applied_repeatable.checksum {
                        println!(
                            "Repeatable migration {} has changed and will be re-applied",
                            label
                        );
                    }
                }
                None => {
                    issue_count += 1;
                    eprintln!(
                        "{}",
                        SchemaInstallerError::MissingMigrationSource {
                            version: label,
                            script_path: applied_repeatable.script_path.clone(),
                        }
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
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        // Propagate failures (bad credentials, missing permissions, ...) rather than
        // reporting them as "everything is pending".
        pool.ensure_migration_table(&config.database_type).await?;

        let applied = pool.get_applied_migrations().await?;
        let applied_versions: HashSet<String> = applied
            .iter()
            .filter(|m| m.status == MigrationStatus::Success.as_str())
            .map(|m| m.version.clone())
            .collect();

        let source_migrations = source.migrations()?;
        let pending = source_migrations
            .iter()
            .any(|m| !applied_versions.contains(&m.version));
        if pending {
            return Ok(true);
        }

        let applied_repeatables = pool.get_applied_repeatables().await?;
        Ok(source
            .repeatable_migrations()?
            .iter()
            .any(|r| repeatable_is_pending(&applied_repeatables, r)))
    }

    pub async fn repair(
        config: &SchemaInstallerConfig,
        source: Box<dyn MigrationSource>,
    ) -> Result<(), SchemaInstallerError> {
        let pool = AnyPool::connect(
            &config.database_type,
            &config.connection_string,
            config.trust_server_certificate,
        )
        .await?;

        pool.ensure_migration_table(&config.database_type).await?;

        let failed = pool.delete_failed_migrations().await?;
        println!("Deleted {} failed migration(s)", failed);

        let stale = pool
            .delete_stale_pending_migrations(STALE_PENDING_THRESHOLD.as_secs() as i64)
            .await?;
        println!(
            "Deleted {} pending migration(s) stuck for over {} seconds (likely abandoned by a crashed process)",
            stale,
            STALE_PENDING_THRESHOLD.as_secs()
        );

        let applied = pool.get_applied_migrations().await?;
        let source_migrations = source.migrations()?;

        for applied_migration in applied {
            if applied_migration.status != MigrationStatus::Success.as_str() {
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

        // Repeatables get the same treatment as versioned migrations, matching Flyway: failed
        // and abandoned-pending rows are cleared, and a changed script's checksum is
        // realigned - so `repair` accepts the edit instead of re-running the script.
        let failed = pool.delete_failed_repeatables().await?;
        println!("Deleted {} failed repeatable migration(s)", failed);

        let stale = pool
            .delete_stale_pending_repeatables(STALE_PENDING_THRESHOLD.as_secs() as i64)
            .await?;
        println!(
            "Deleted {} pending repeatable migration(s) stuck for over {} seconds (likely abandoned by a crashed process)",
            stale,
            STALE_PENDING_THRESHOLD.as_secs()
        );

        let source_repeatables = source.repeatable_migrations()?;
        for applied_repeatable in pool.get_applied_repeatables().await? {
            if applied_repeatable.status != MigrationStatus::Success.as_str() {
                continue;
            }
            if let Some(source_repeatable) = source_repeatables
                .iter()
                .find(|r| r.name == applied_repeatable.name)
            {
                let checksum = compute_checksum(&source_repeatable.sql);
                if checksum != applied_repeatable.checksum {
                    pool.update_repeatable_checksum(applied_repeatable.id, &checksum)
                        .await?;
                    println!(
                        "Updated checksum for repeatable migration: R__{}",
                        applied_repeatable.name
                    );
                }
            }
        }

        Ok(())
    }
}

/// Which tracking table a script's status is recorded in.
#[derive(Clone, Copy)]
enum Tracking {
    Versioned,
    Repeatable,
}

async fn execute_migration(
    pool: &AnyPool,
    database_type: &GeneratorType,
    migration_id: i64,
    migration: &Migration,
) -> Result<i64, SchemaInstallerError> {
    execute_script(
        pool,
        database_type,
        Tracking::Versioned,
        migration_id,
        &migration.sql,
        migration.transactional,
    )
    .await
}

async fn execute_repeatable(
    pool: &AnyPool,
    database_type: &GeneratorType,
    repeatable_id: i64,
    repeatable: &RepeatableMigration,
) -> Result<i64, SchemaInstallerError> {
    execute_script(
        pool,
        database_type,
        Tracking::Repeatable,
        repeatable_id,
        &repeatable.sql,
        repeatable.transactional,
    )
    .await
}

async fn execute_script(
    pool: &AnyPool,
    database_type: &GeneratorType,
    tracking: Tracking,
    id: i64,
    sql: &str,
    transactional: bool,
) -> Result<i64, SchemaInstallerError> {
    let statements = crate::sql_split::split_sql_statements(sql, database_type)?;
    if !transactional {
        // Autocommit: the statements can't be atomic with the tracking row (M18), so the
        // success update is a separate step after they have all run.
        let elapsed_ms = pool
            .execute_migration_non_transactional(&statements)
            .await?;
        let success = MigrationStatus::Success.as_str();
        match tracking {
            Tracking::Versioned => {
                pool.update_migration_status(id, success, elapsed_ms)
                    .await?
            }
            Tracking::Repeatable => {
                pool.update_repeatable_status(id, success, elapsed_ms)
                    .await?
            }
        }
        return Ok(elapsed_ms);
    }
    // All statements in a script, plus the tracking row's "success" update, commit or roll
    // back together as one transaction - see `execute_migration_transactional` for why
    // (BUGS_AND_GAPS H9).
    match tracking {
        Tracking::Versioned => pool.execute_migration_transactional(&statements, id).await,
        Tracking::Repeatable => pool.execute_repeatable_transactional(&statements, id).await,
    }
}

/// Rejects, before anything is executed, any transactional migration whose script contains
/// its own transaction control (M18) - see `find_transaction_control` for why it would
/// wedge the migration. Scripts that need to manage their own transactions must opt out
/// with `executeInTransaction=false` in a `<script>.conf`.
fn validate_migration_scripts(
    migrations: &[Migration],
    repeatables: &[RepeatableMigration],
    database_type: &GeneratorType,
) -> Result<(), SchemaInstallerError> {
    let scripts = migrations
        .iter()
        .map(|m| {
            (
                format!("Migration {}", m.version),
                &m.script_path,
                &m.sql,
                m.transactional,
            )
        })
        .chain(repeatables.iter().map(|r| {
            (
                format!("Repeatable migration {}", r.name),
                &r.script_path,
                &r.sql,
                r.transactional,
            )
        }));

    for (label, script_path, sql, _) in scripts.filter(|script| script.3) {
        let statements = crate::sql_split::split_sql_statements(sql, database_type)?;
        if let Some(statement) =
            crate::sql_split::find_transaction_control(&statements, database_type)
        {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "{} ({}) contains its own transaction control (`{}`), but migrations run inside a transaction managed by the installer; remove it, or set `executeInTransaction=false` in {}.conf to run the script without a wrapping transaction",
                label, script_path, statement, script_path
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(status: &str, sql: &str) -> AppliedRepeatableMigration {
        AppliedRepeatableMigration {
            id: 1,
            name: "v".to_string(),
            script_path: "R__v.sql".to_string(),
            checksum: compute_checksum(sql),
            execution_time_ms: 0,
            installed_at: String::new(),
            status: status.to_string(),
            tool_version: String::new(),
        }
    }

    fn source(sql: &str) -> RepeatableMigration {
        RepeatableMigration {
            name: "v".to_string(),
            script_path: "R__v.sql".to_string(),
            sql: sql.to_string(),
            transactional: true,
        }
    }

    #[test]
    fn repeatable_info_status_distinguishes_pending_applied_and_outdated() {
        let s = source("select 1;");
        assert_eq!(repeatable_info_status(None, Some(&s)), "Pending");
        assert_eq!(
            repeatable_info_status(Some(&applied("success", "select 1;")), Some(&s)),
            "success"
        );
        assert_eq!(
            repeatable_info_status(Some(&applied("success", "select 0;")), Some(&s)),
            "Outdated"
        );
        assert_eq!(
            repeatable_info_status(Some(&applied("failed", "select 0;")), Some(&s)),
            "failed"
        );
        // Applied but the file is gone: shown as recorded.
        assert_eq!(
            repeatable_info_status(Some(&applied("success", "select 1;")), None),
            "success"
        );
    }

    #[test]
    fn repeatable_is_pending_when_new_changed_or_not_successful() {
        let s = source("select 1;");
        assert!(repeatable_is_pending(&[], &s));
        assert!(!repeatable_is_pending(
            &[applied("success", "select 1;")],
            &s
        ));
        assert!(repeatable_is_pending(
            &[applied("success", "select 0;")],
            &s
        ));
        assert!(repeatable_is_pending(&[applied("failed", "select 1;")], &s));
        assert!(repeatable_is_pending(
            &[applied("pending", "select 1;")],
            &s
        ));
    }
}
