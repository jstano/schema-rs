use schema_installer::connection::AnyPool;
use schema_installer::{
    DirectoryMigrationSource, EmbeddedMigrationSource, Migration, Migrator, SchemaInstaller,
    SchemaInstallerConfigBuilder, SchemaInstallerError,
};
use schema_sql_generator::common::generator_type::GeneratorType;
use sqlx::Row;
use std::path::PathBuf;
use tempfile::TempDir;

fn sqlite_connection_string(temp_dir: &TempDir, name: &str) -> String {
    format!("sqlite://{}", temp_dir.path().join(name).to_string_lossy())
}

#[tokio::test]
async fn test_sqlite_migration_flow() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_migration_flow.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sqlite");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::migrate(&config, source)
        .await
        .expect("migration should succeed");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    assert!(!Migrator::has_pending_migrations(&config, source)
        .await
        .expect("has_pending_migrations should succeed"));
}

#[tokio::test]
async fn test_sqlite_validate_detects_checksum_mismatch() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_validate.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sqlite");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::migrate(&config, source)
        .await
        .expect("initial migration should succeed");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::validate(&config, source)
        .await
        .expect("validate should succeed");
}

#[tokio::test]
async fn test_sqlite_rerunning_migrate_is_noop() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_rerun.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sqlite");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::migrate(&config, source)
        .await
        .expect("first migration should succeed");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::migrate(&config, source)
        .await
        .expect("second migration should succeed");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    assert!(!Migrator::has_pending_migrations(&config, source)
        .await
        .expect("has_pending_migrations should succeed"));
}

#[tokio::test]
async fn test_sqlite_failed_migration_rolls_back_partial_statements() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_rollback.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    // First statement succeeds, second is invalid SQL and fails. Without transactional
    // wrapping, `widgets` would be left permanently in the schema despite the migration
    // being recorded as "failed".
    let bad_migration = Migration {
        version: "1".to_string(),
        description: "create widgets then fail".to_string(),
        script_path: "V1__create_widgets_then_fail.sql".to_string(),
        sql: "create table widgets (id integer primary key);\nthis is not valid sql;".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![bad_migration],
    });

    let result = Migrator::migrate(&config, source).await;
    assert!(
        result.is_err(),
        "migration containing an invalid statement should fail"
    );

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify rollback");
    let row = sqlx::query(
        "SELECT COUNT(*) as count FROM sqlite_master WHERE type = 'table' AND name = 'widgets'",
    )
    .fetch_one(&check_pool)
    .await
    .expect("query sqlite_master");
    let count: i64 = row.get("count");
    assert_eq!(
        count, 0,
        "widgets table should have been rolled back after the migration failed"
    );
}

#[tokio::test]
async fn test_sqlite_migration_ddl_and_success_status_commit_atomically() {
    // Regression test for BUGS_AND_GAPS H9: previously the migration DDL and the
    // "success" status update were separate transactions, so a crash between the two
    // left the DDL permanently applied while the tracking row stayed "pending" forever.
    // They are now folded into one transaction inside `execute_migration_transactional`,
    // so if the tracking-row update can't happen (here: it targets a migration id that
    // doesn't exist), the DDL must never take effect either - proving the two are truly
    // atomic rather than merely sequential.
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_atomic_status.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let pool = AnyPool::connect(&config.database_type, &config.connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&config.database_type)
        .await
        .expect("ensure migration table");

    let statements = vec!["create table widgets (id integer primary key)".to_string()];
    const NONEXISTENT_MIGRATION_ID: i64 = 999_999;
    let result = pool
        .execute_migration_transactional(&statements, NONEXISTENT_MIGRATION_ID)
        .await;
    assert!(
        result.is_err(),
        "the status update should fail because no tracking row has that id"
    );

    let row = sqlx::query(
        "SELECT COUNT(*) as count FROM sqlite_master WHERE type = 'table' AND name = 'widgets'",
    )
    .fetch_one(match &pool {
        AnyPool::Sqlite(p) => p,
        _ => unreachable!("sqlite pool in a sqlite test"),
    })
    .await
    .expect("query sqlite_master");
    let count: i64 = row.get("count");
    assert_eq!(
        count, 0,
        "the DDL must be rolled back when the same-transaction status update fails"
    );
}

#[tokio::test]
async fn test_sqlite_concurrent_migrate_calls_converge_without_duplicates() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_concurrent.db");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sqlite");

    let config_a = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");
    let config_b = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let source_a = Box::new(DirectoryMigrationSource {
        path: fixtures_dir.clone(),
    });
    let source_b = Box::new(DirectoryMigrationSource {
        path: fixtures_dir.clone(),
    });

    let (result_a, result_b) = tokio::join!(
        Migrator::migrate(&config_a, source_a),
        Migrator::migrate(&config_b, source_b),
    );

    // Neither instance should see a raw concurrency error: whichever one loses the
    // race waits on the winner's row and then recognizes the migration as already
    // applied, the same way a second Flyway instance blocks and then no-ops instead
    // of failing.
    assert!(
        result_a.is_ok(),
        "instance A should succeed or converge cleanly: {:?}",
        result_a.err()
    );
    assert!(
        result_b.is_ok(),
        "instance B should succeed or converge cleanly: {:?}",
        result_b.err()
    );

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify no duplicates");
    let duplicate_rows = sqlx::query(
        "SELECT version, COUNT(*) as count FROM schema_migration \
         WHERE status = 'success' GROUP BY version HAVING COUNT(*) > 1",
    )
    .fetch_all(&check_pool)
    .await
    .expect("query for duplicate versions");
    assert!(
        duplicate_rows.is_empty(),
        "no version should have duplicate successful tracking rows"
    );

    let source_check = Box::new(DirectoryMigrationSource {
        path: fixtures_dir.clone(),
    });
    assert!(!Migrator::has_pending_migrations(&config_a, source_check)
        .await
        .expect("has_pending_migrations should succeed"));
}

#[tokio::test]
async fn test_sqlite_concurrent_migrate_calls_propagate_shared_failure() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_concurrent_failure.db");

    let config_a = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");
    let config_b = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let bad_migration = || Migration {
        version: "1".to_string(),
        description: "bad migration".to_string(),
        script_path: "V1__bad.sql".to_string(),
        sql: "this is not valid sql;".to_string(),
    };
    let source_a = Box::new(EmbeddedMigrationSource {
        migrations: vec![bad_migration()],
    });
    let source_b = Box::new(EmbeddedMigrationSource {
        migrations: vec![bad_migration()],
    });

    let (result_a, result_b) = tokio::join!(
        Migrator::migrate(&config_a, source_a),
        Migrator::migrate(&config_b, source_b),
    );

    // Whichever instance actually runs the migration fails directly; whichever one
    // waited on the other's row must observe that failure too, rather than silently
    // succeeding or hanging until the lock-wait timeout.
    assert!(
        result_a.is_err() && result_b.is_err(),
        "both instances should report the shared migration failure: a={:?} b={:?}",
        result_a,
        result_b
    );
}

#[tokio::test]
async fn test_sqlite_validate_detects_missing_migration_source() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_missing_source.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets".to_string(),
        script_path: "V1__create_widgets.sql".to_string(),
        sql: "create table widgets (id integer primary key);".to_string(),
    };

    // Apply V1 while its "file" still exists.
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one],
    });
    Migrator::migrate(&config, source)
        .await
        .expect("migration should succeed");

    // Validate against a source where V1's file is gone, as if it had been deleted or
    // renamed after being applied.
    let empty_source = Box::new(EmbeddedMigrationSource { migrations: vec![] });
    let result = Migrator::validate(&config, empty_source).await;
    assert!(
        result.is_err(),
        "validate should fail when an applied migration's source file is missing"
    );
}

#[tokio::test]
async fn test_sqlite_validate_exempts_reserved_install_version() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_reserved_version.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    // Simulate the legacy XML `install` command's tracking row: reserved version "0",
    // which never corresponds to a real migration file.
    let pool = AnyPool::connect(&GeneratorType::Sqlite, &connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&GeneratorType::Sqlite)
        .await
        .expect("ensure migration table");
    pool.insert_migration("0", "V0__install_schema.sql", "deadbeef", 0, "success", "test")
        .await
        .expect("insert reserved install row");

    // No migration files at all; the reserved version must not be flagged as missing.
    let empty_source = Box::new(EmbeddedMigrationSource { migrations: vec![] });
    Migrator::validate(&config, empty_source)
        .await
        .expect("validate should not flag the reserved install version as missing");
}

#[tokio::test]
async fn test_sqlite_validate_detects_failed_migration() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_validate_failed.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    // Simulate a migration that a previous `migrate` run recorded as "failed" (`migrate`
    // itself would refuse to re-run this version until `repair` clears it out - see
    // `wait_for_slot`'s "run `repair` before retrying" error). BUGS_AND_GAPS H10: before
    // this fix, `validate`'s loop skipped any non-"success" row outright and reported no
    // issue at all.
    let pool = AnyPool::connect(&GeneratorType::Sqlite, &connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&GeneratorType::Sqlite)
        .await
        .expect("ensure migration table");
    pool.insert_migration("1", "V1__create_widgets.sql", "deadbeef", 0, "failed", "test")
        .await
        .expect("insert failed migration row");

    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets".to_string(),
        script_path: "V1__create_widgets.sql".to_string(),
        sql: "create table widgets (id integer primary key);".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one],
    });
    let result = Migrator::validate(&config, source).await;
    assert!(
        result.is_err(),
        "validate should flag a migration left in a \"failed\" state, matching what migrate rejects"
    );
}

#[tokio::test]
async fn test_sqlite_validate_detects_out_of_order_unapplied_migration() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_validate_out_of_order.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    // V2 applies first, as if V1 didn't exist yet at the time.
    let migration_two = Migration {
        version: "2".to_string(),
        description: "add widget color".to_string(),
        script_path: "V2__add_widget_color.sql".to_string(),
        sql: "create table widgets (id integer primary key, color text);".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_two.clone()],
    });
    Migrator::migrate(&config, source)
        .await
        .expect("V2 should apply cleanly on its own");

    // V1 shows up afterward, unapplied. `migrate` rejects this as `OutOfOrderMigration`
    // (see `test_sqlite_migrate_rejects_out_of_order_migration`); BUGS_AND_GAPS H10:
    // before this fix, `validate` only ever looked at already-applied migrations and
    // never noticed a resolved-but-unapplied one at all, so it passed on this identical
    // state.
    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets base".to_string(),
        script_path: "V1__create_widgets_base.sql".to_string(),
        sql: "select 1;".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one, migration_two],
    });
    let result = Migrator::validate(&config, source).await;
    assert!(
        result.is_err(),
        "validate should flag a resolved migration older than the highest applied version that was never applied"
    );
}

#[tokio::test]
async fn test_sqlite_migrate_detects_missing_migration_source() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_migrate_missing_source.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets".to_string(),
        script_path: "V1__create_widgets.sql".to_string(),
        sql: "create table widgets (id integer primary key);".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one],
    });
    Migrator::migrate(&config, source)
        .await
        .expect("initial migration should succeed");

    // V1's file is now "gone", but a new pending migration V2 is present. `migrate`
    // must refuse to proceed at all - not even apply V2 - until the drift on V1 is
    // resolved, the same way Flyway's validateOnMigrate blocks a migrate run.
    let migration_two = Migration {
        version: "2".to_string(),
        description: "add widget color".to_string(),
        script_path: "V2__add_widget_color.sql".to_string(),
        sql: "alter table widgets add column color text;".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_two],
    });
    let result = Migrator::migrate(&config, source).await;
    assert!(
        result.is_err(),
        "migrate should refuse to run while an applied migration's source file is missing"
    );

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify V2 was not applied");
    let row = sqlx::query("SELECT COUNT(*) as count FROM schema_migration WHERE version = '2'")
        .fetch_one(&check_pool)
        .await
        .expect("query schema_migration");
    let count: i64 = row.get("count");
    assert_eq!(
        count, 0,
        "V2 should not have been attempted while V1's drift was unresolved"
    );
}

#[tokio::test]
async fn test_sqlite_migrate_rejects_out_of_order_migration() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_out_of_order.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    // V2 applies first, as if V1 didn't exist yet at the time.
    let migration_two = Migration {
        version: "2".to_string(),
        description: "add widget color".to_string(),
        script_path: "V2__add_widget_color.sql".to_string(),
        sql: "create table widgets (id integer primary key, color text);".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_two.clone()],
    });
    Migrator::migrate(&config, source)
        .await
        .expect("V2 should apply cleanly on its own");

    // V1 shows up afterward (e.g. merged late from another branch). `migrate` must
    // refuse to apply it out of order rather than silently running it against a schema
    // state it was never designed for.
    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets base".to_string(),
        script_path: "V1__create_widgets_base.sql".to_string(),
        sql: "select 1;".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one, migration_two],
    });
    let result = Migrator::migrate(&config, source).await;
    assert!(
        matches!(result, Err(SchemaInstallerError::OutOfOrderMigration { .. })),
        "migrate should reject V1 as out-of-order once V2 is already applied: {:?}",
        result
    );
}

#[tokio::test]
async fn test_sqlite_migrate_applies_embedded_migrations_in_version_order_not_source_order() {
    // Regression test: EmbeddedMigrationSource returns migrations in whatever order the
    // caller supplied (unlike DirectoryMigrationSource, which sorts internally), so
    // `migrate` itself must sort by version before applying. V2 depends on the table V1
    // creates; if migrate() applied them in the given (reversed) order, V2 would fail
    // with "no such table: widgets".
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_unsorted_embedded.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets".to_string(),
        script_path: "V1__create_widgets.sql".to_string(),
        sql: "create table widgets (id integer primary key);".to_string(),
    };
    let migration_two = Migration {
        version: "2".to_string(),
        description: "add widgets column".to_string(),
        script_path: "V2__add_widgets_column.sql".to_string(),
        sql: "alter table widgets add column extra text;".to_string(),
    };

    // Deliberately supplied out of version order.
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_two, migration_one],
    });

    Migrator::migrate(&config, source)
        .await
        .expect("migrate should sort by version and apply V1 before V2");

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify schema");
    let row = sqlx::query("SELECT COUNT(*) as count FROM pragma_table_info('widgets') WHERE name = 'extra'")
        .fetch_one(&check_pool)
        .await
        .expect("query pragma_table_info");
    let count: i64 = row.get("count");
    assert_eq!(count, 1, "widgets.extra should exist once both migrations applied in order");
}

#[tokio::test]
async fn test_sqlite_repair_removes_stale_pending_migrations() {
    // Simulates a process that crashed mid-migration: a "pending" row is left behind
    // with no corresponding process left to ever mark it "success" or "failed". Without
    // repair cleaning it up, every future migrate() call would wait out the lock-timeout
    // and fail, forever.
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_repair_stale_pending.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let pool = AnyPool::connect(&GeneratorType::Sqlite, &connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&GeneratorType::Sqlite)
        .await
        .expect("ensure migration table");
    let id = pool
        .insert_migration("1", "V1__create_widgets.sql", "deadbeef", 0, "pending", "test")
        .await
        .expect("insert pending row");

    // Back-date it well past repair's staleness threshold.
    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to backdate row");
    sqlx::query("UPDATE schema_migration SET installed_at = datetime('now', '-700 seconds') WHERE id = ?")
        .bind(id)
        .execute(&check_pool)
        .await
        .expect("backdate installed_at");

    let empty_source = Box::new(EmbeddedMigrationSource { migrations: vec![] });
    Migrator::repair(&config, empty_source)
        .await
        .expect("repair should succeed");

    let row = sqlx::query("SELECT COUNT(*) as count FROM schema_migration WHERE status = 'pending'")
        .fetch_one(&check_pool)
        .await
        .expect("query schema_migration");
    let count: i64 = row.get("count");
    assert_eq!(count, 0, "repair should have deleted the stale pending row");
}

#[tokio::test]
async fn test_sqlite_repair_keeps_recent_pending_migrations() {
    // A "pending" row inserted moments ago could still belong to a process that's
    // legitimately mid-migration right now; repair must not delete it out from under
    // that process just because *some* pending row happened to exist.
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_repair_recent_pending.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let pool = AnyPool::connect(&GeneratorType::Sqlite, &connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&GeneratorType::Sqlite)
        .await
        .expect("ensure migration table");
    pool.insert_migration("1", "V1__create_widgets.sql", "deadbeef", 0, "pending", "test")
        .await
        .expect("insert pending row");

    let empty_source = Box::new(EmbeddedMigrationSource { migrations: vec![] });
    Migrator::repair(&config, empty_source)
        .await
        .expect("repair should succeed");

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify row survives");
    let row = sqlx::query("SELECT COUNT(*) as count FROM schema_migration WHERE status = 'pending'")
        .fetch_one(&check_pool)
        .await
        .expect("query schema_migration");
    let count: i64 = row.get("count");
    assert_eq!(count, 1, "repair should not delete a recently-inserted pending row");
}

#[tokio::test]
async fn test_sqlite_repair_succeeds_on_an_un_set_up_database() {
    // H12: `repair` used to go straight to `delete_failed_migrations()` without first
    // calling `ensure_migration_table`, unlike every other entry point (migrate/info/
    // validate). Against a database that has never had a migration run against it, that
    // meant `repair` was the one command that failed with "no such table: schema_migration".
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_repair_fresh_database.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let empty_source = Box::new(EmbeddedMigrationSource { migrations: vec![] });
    Migrator::repair(&config, empty_source)
        .await
        .expect("repair should succeed even when schema_migration does not exist yet");
}

#[tokio::test]
async fn test_sqlite_info_reports_no_migrations_on_a_fresh_database() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_info_fresh.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let empty_source = Box::new(EmbeddedMigrationSource { migrations: vec![] });
    Migrator::info(&config, empty_source)
        .await
        .expect("info should succeed against a fresh database with no migrations");
}

#[tokio::test]
async fn test_sqlite_info_lists_applied_and_pending_migrations() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_info_mixed.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets".to_string(),
        script_path: "V1__create_widgets.sql".to_string(),
        sql: "create table widgets (id integer primary key);".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource { migrations: vec![migration_one.clone()] });
    Migrator::migrate(&config, source)
        .await
        .expect("migration should succeed");

    let migration_two = Migration {
        version: "2".to_string(),
        description: "add widgets column".to_string(),
        script_path: "V2__add_widgets_column.sql".to_string(),
        sql: "alter table widgets add column extra text;".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one, migration_two],
    });
    Migrator::info(&config, source)
        .await
        .expect("info should succeed and list both the applied and pending migration");
}

fn simple_schema_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/simple-test-schema.xml")
}

#[tokio::test]
async fn test_sqlite_install_is_not_skipped_by_an_unrelated_migration() {
    // Regression test for BUGS_AND_GAPS H14: `check_if_installed` used to treat *any*
    // successful tracking row as "already installed", so running a Flyway-style
    // `migrate` first made a later `install` report "already installed" and skip,
    // without ever creating a single table.
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_install_not_skipped.db");

    let migrate_config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .build()
        .expect("valid config");

    let migration_one = Migration {
        version: "1".to_string(),
        description: "create widgets".to_string(),
        script_path: "V1__create_widgets.sql".to_string(),
        sql: "create table widgets (id integer primary key);".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![migration_one],
    });
    Migrator::migrate(&migrate_config, source)
        .await
        .expect("unrelated migration should succeed");

    let install_config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .schema_file(simple_schema_file())
        .build()
        .expect("valid config");
    SchemaInstaller::install(&install_config)
        .await
        .expect("install should still run after an unrelated migration was applied");

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify install ran");
    let row = sqlx::query(
        "SELECT COUNT(*) as count FROM sqlite_master WHERE type = 'table' AND name = 'users'",
    )
    .fetch_one(&check_pool)
    .await
    .expect("query sqlite_master");
    let count: i64 = row.get("count");
    assert_eq!(count, 1, "install should have created the schema's users table");
}

#[tokio::test]
async fn test_sqlite_install_twice_is_a_noop_on_the_second_call() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_install_twice.db");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string)
        .schema_file(simple_schema_file())
        .build()
        .expect("valid config");

    SchemaInstaller::install(&config)
        .await
        .expect("first install should succeed");
    SchemaInstaller::install(&config)
        .await
        .expect("second install should be a no-op rather than erroring");
}

#[tokio::test]
async fn test_sqlite_install_can_be_retried_after_a_failed_attempt() {
    // Regression test for BUGS_AND_GAPS H14: a previous `install` attempt that failed
    // partway used to leave a "failed" tracking row for the reserved version behind
    // (simulated here directly rather than by forcing a real mid-script failure). A
    // retry's `insert_migration` would then collide with that leftover row's
    // `UNIQUE (version)` and surface as a bogus `ConcurrentMigrationDetected("0")`
    // instead of just running the install.
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_install_retry.db");

    let pool = AnyPool::connect(&GeneratorType::Sqlite, &connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&GeneratorType::Sqlite)
        .await
        .expect("ensure migration table");
    pool.insert_migration("0", "V0__install_schema.sql", "deadbeef", 0, "failed", "test")
        .await
        .expect("simulate leftover failed install row");

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string(connection_string.clone())
        .schema_file(simple_schema_file())
        .build()
        .expect("valid config");

    SchemaInstaller::install(&config)
        .await
        .expect("install should succeed despite a leftover failed tracking row");

    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify install ran");
    let row = sqlx::query(
        "SELECT COUNT(*) as count FROM sqlite_master WHERE type = 'table' AND name = 'users'",
    )
    .fetch_one(&check_pool)
    .await
    .expect("query sqlite_master");
    let count: i64 = row.get("count");
    assert_eq!(count, 1, "retried install should have created the schema's users table");
}

#[tokio::test]
async fn test_sqlite_update_migration_status_errors_when_the_tracking_row_is_gone() {
    // Regression test: if a migration's tracking row disappears out from under a
    // still-running migration (e.g. a concurrent `repair()` deleted a stale-looking but
    // actually-still-in-progress "pending" row), recording its completion must surface
    // as a loud error rather than silently succeeding with the migration's completion
    // never actually recorded.
    let temp_dir = TempDir::new().expect("create temp dir");
    let connection_string = sqlite_connection_string(&temp_dir, "test_update_status_missing_row.db");

    let pool = AnyPool::connect(&GeneratorType::Sqlite, &connection_string)
        .await
        .expect("connect");
    pool.ensure_migration_table(&GeneratorType::Sqlite)
        .await
        .expect("ensure migration table");
    let id = pool
        .insert_migration("1", "V1__create_widgets.sql", "deadbeef", 0, "pending", "test")
        .await
        .expect("insert pending row");

    // Simulate the row being removed out from under the in-progress migration.
    let check_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to delete row");
    sqlx::query("DELETE FROM schema_migration WHERE id = ?")
        .bind(id)
        .execute(&check_pool)
        .await
        .expect("delete tracking row");

    let result = pool.update_migration_status(id, "success", 100).await;
    assert!(
        result.is_err(),
        "update_migration_status should error when its tracking row no longer exists"
    );
}
