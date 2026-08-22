use schema_installer::connection::AnyPool;
use schema_installer::{
    DirectoryMigrationSource, EmbeddedMigrationSource, Migration, Migrator, SchemaInstallerConfigBuilder,
    SchemaInstallerError,
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
