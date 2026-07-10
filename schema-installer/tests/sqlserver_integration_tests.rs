use schema_installer::{DirectoryMigrationSource, Migrator, SchemaInstallerConfigBuilder};
use schema_sql_generator::common::generator_type::GeneratorType;
use std::path::PathBuf;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mssql_server::MssqlServer;

// SQL Server's Linux container crashes under QEMU emulation (Apple Silicon + Colima/Docker
// Desktop), so these are ignored by default. They pass on native x86_64 hosts/CI runners:
// cargo test -p schema-installer --test sqlserver_integration_tests -- --ignored

#[tokio::test]
#[ignore]
async fn test_sqlserver_migration_flow() {
    let mssql = MssqlServer::default().start().await.expect("mssql container should start");
    let port = mssql.get_host_port_ipv4(1433).await.expect("get mapped port");

    let connection_string = format!(
        "Server=localhost,{};Database=master;User Id=sa;Password=yourStrong(!)Password;TrustServerCertificate=true;",
        port
    );

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::SqlServer)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sqlserver");

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
#[ignore]
async fn test_sqlserver_validate_detects_checksum_mismatch() {
    let mssql = MssqlServer::default().start().await.expect("mssql container should start");
    let port = mssql.get_host_port_ipv4(1433).await.expect("get mapped port");

    let connection_string = format!(
        "Server=localhost,{};Database=master;User Id=sa;Password=yourStrong(!)Password;TrustServerCertificate=true;",
        port
    );

    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::SqlServer)
        .connection_string(connection_string)
        .build()
        .expect("valid config");

    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sqlserver");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::migrate(&config, source)
        .await
        .expect("initial migration should succeed");

    let source = Box::new(DirectoryMigrationSource { path: fixtures_dir.clone() });
    Migrator::validate(&config, source)
        .await
        .expect("validate should succeed");
}
