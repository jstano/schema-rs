use schema_installer::{DirectoryMigrationSource, Migrator, SchemaInstallerConfigBuilder};
use schema_sql_generator::common::generator_type::GeneratorType;
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
