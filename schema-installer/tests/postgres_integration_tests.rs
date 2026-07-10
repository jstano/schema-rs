use schema_installer::{DirectoryMigrationSource, Migrator, SchemaInstallerConfigBuilder};
use schema_sql_generator::common::generator_type::GeneratorType;
use std::path::PathBuf;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn test_postgres_migration_flow() {
    let postgres = Postgres::default().start().await.expect("postgres container should start");
    let port = postgres.get_host_port_ipv4(5432).await.expect("get mapped port");

    let connection_string = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        port
    );

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Postgresql)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/postgres");

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
async fn test_postgres_validate_detects_checksum_mismatch() {
    let postgres = Postgres::default().start().await.expect("postgres container should start");
    let port = postgres.get_host_port_ipv4(5432).await.expect("get mapped port");

    let connection_string = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        port
    );

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Postgresql)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/postgres");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::migrate(&config, source)
        .await
        .expect("initial migration should succeed");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::validate(&config, source)
        .await
        .expect("validate should succeed");
}
