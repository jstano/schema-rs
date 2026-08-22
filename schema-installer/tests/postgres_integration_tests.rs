use schema_installer::{
    DirectoryMigrationSource, EmbeddedMigrationSource, Migration, Migrator, SchemaInstallerConfigBuilder,
};
use schema_sql_generator::common::generator_type::GeneratorType;
use sqlx::Row;
use std::path::PathBuf;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

fn docker_tests_enabled() -> bool {
    std::env::var("RUN_DOCKER_TESTS").is_ok()
}

#[tokio::test]
async fn test_postgres_migration_flow() {
    if !docker_tests_enabled() {
        eprintln!("skipping test_postgres_migration_flow: set RUN_DOCKER_TESTS=1 to run");
        return;
    }

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
    if !docker_tests_enabled() {
        eprintln!("skipping test_postgres_validate_detects_checksum_mismatch: set RUN_DOCKER_TESTS=1 to run");
        return;
    }

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

#[tokio::test]
async fn test_postgres_failed_migration_rolls_back_partial_statements() {
    if !docker_tests_enabled() {
        eprintln!("skipping test_postgres_failed_migration_rolls_back_partial_statements: set RUN_DOCKER_TESTS=1 to run");
        return;
    }

    let postgres = Postgres::default().start().await.expect("postgres container should start");
    let port = postgres.get_host_port_ipv4(5432).await.expect("get mapped port");

    let connection_string = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        port
    );

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Postgresql)
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
        sql: "create table widgets (id bigserial primary key);\nthis is not valid sql;".to_string(),
    };
    let source = Box::new(EmbeddedMigrationSource {
        migrations: vec![bad_migration],
    });

    let result = Migrator::migrate(&config, source).await;
    assert!(
        result.is_err(),
        "migration containing an invalid statement should fail"
    );

    let check_pool = sqlx::postgres::PgPoolOptions::new()
        .connect(&connection_string)
        .await
        .expect("connect to verify rollback");
    let row = sqlx::query(
        "SELECT COUNT(*) as count FROM information_schema.tables WHERE table_name = 'widgets'",
    )
    .fetch_one(&check_pool)
    .await
    .expect("query information_schema.tables");
    let count: i64 = row.get("count");
    assert_eq!(
        count, 0,
        "widgets table should have been rolled back after the migration failed"
    );
}
