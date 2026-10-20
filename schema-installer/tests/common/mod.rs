//! A dialect-neutral end-to-end scenario for repeatable (`R__`) migrations, run by the Postgres
//! and SQL Server suites. The SQLite equivalents live in `sqlite_repeatable_tests.rs`; this
//! exists because the claim/re-run SQL (placeholders, `now()` vs `SYSUTCDATETIME()`, rows
//! affected) differs per driver and only a real server exercises it.

#![allow(dead_code)] // each suite constructs only its own `LogReader` variant

use schema_installer::{
    DirectoryMigrationSource, Migrator, SchemaInstallerConfig, SchemaInstallerError,
};
use sqlx::Row;
use std::path::Path;
use tempfile::TempDir;
use tokio_util::compat::TokioAsyncWriteCompatExt;

/// How the scenario reads back the `log` table the migrations write to.
pub enum LogReader {
    Postgres(String),
    SqlServer(String),
}

impl LogReader {
    async fn entries(&self) -> Vec<String> {
        match self {
            LogReader::Postgres(url) => {
                let pool = sqlx::PgPool::connect(url).await.expect("connect");
                let rows = sqlx::query("select entry from log order by seq")
                    .fetch_all(&pool)
                    .await
                    .expect("read log");
                rows.iter().map(|r| r.get::<String, _>("entry")).collect()
            }
            LogReader::SqlServer(ado) => {
                let config = tiberius::Config::from_ado_string(ado).expect("ado string");
                let tcp = tokio::net::TcpStream::connect(config.get_addr())
                    .await
                    .expect("tcp");
                let mut client = tiberius::Client::connect(config, tcp.compat_write())
                    .await
                    .expect("login");
                let rows = client
                    .simple_query("select entry from log order by seq")
                    .await
                    .expect("read log")
                    .into_first_result()
                    .await
                    .expect("rows");
                rows.iter()
                    .map(|r| r.get::<&str, _>(0).expect("entry").to_string())
                    .collect()
            }
        }
    }
}

fn log_entry(entry: &str) -> String {
    // Portable across Postgres and SQL Server; `seq` records insertion order.
    format!("insert into log (entry, seq) select '{entry}', count(*) from log;")
}

fn source(dir: &Path) -> Box<DirectoryMigrationSource> {
    Box::new(DirectoryMigrationSource {
        path: dir.to_path_buf(),
    })
}

async fn applied(
    config: &SchemaInstallerConfig,
) -> Vec<schema_installer::AppliedRepeatableMigration> {
    let pool = schema_installer::connection::AnyPool::connect(
        &config.database_type,
        &config.connection_string,
        config.trust_server_certificate,
    )
    .await
    .expect("connect");
    pool.get_applied_repeatables().await.expect("applied")
}

pub async fn run_repeatable_scenario(config: &SchemaInstallerConfig, reader: LogReader) {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();
    let write = |name: &str, sql: &str| std::fs::write(dir.join(name), sql).unwrap();
    let migrate = || Migrator::migrate(config, source(dir));
    let has_pending = || async {
        Migrator::has_pending_migrations(config, source(dir))
            .await
            .expect("has_pending_migrations")
    };

    write(
        "V1__create_log.sql",
        "create table log (entry varchar(100) not null, seq integer not null);",
    );
    // Written out of name order on purpose.
    write("R__b.sql", &log_entry("b"));
    write("R__a.sql", &log_entry("a"));

    // 1. Repeatables run after the versioned migration, in name order.
    migrate().await.expect("first migrate");
    assert_eq!(reader.entries().await, ["a", "b"]);
    let rows = applied(config).await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.status == "success"));
    assert!(!has_pending().await);

    // 2. Unchanged: a no-op.
    migrate().await.expect("second migrate");
    assert_eq!(reader.entries().await, ["a", "b"]);

    // 3. Changed: re-runs, tracked in place (this is the claim `UPDATE` on a real server).
    let before = applied(config)
        .await
        .into_iter()
        .find(|r| r.name == "a")
        .unwrap();
    write("R__a.sql", &log_entry("a2"));
    assert!(has_pending().await);
    migrate().await.expect("migrate after edit");
    assert_eq!(reader.entries().await, ["a", "b", "a2"]);
    let rows = applied(config).await;
    assert_eq!(rows.len(), 2, "one row per repeatable");
    let after = rows.iter().find(|r| r.name == "a").unwrap();
    assert_eq!(after.id, before.id);
    assert_ne!(after.checksum, before.checksum);
    assert_eq!(after.status, "success");
    assert!(!after.installed_at.is_empty());

    // 4. info / validate work; a changed script is not a validation error.
    Migrator::info(config, source(dir)).await.expect("info");
    write("R__a.sql", &log_entry("a3"));
    Migrator::validate(config, source(dir))
        .await
        .expect("validate tolerates a changed repeatable");

    // 5. repair accepts the edit instead of re-running it.
    Migrator::repair(config, source(dir)).await.expect("repair");
    assert!(!has_pending().await);
    migrate().await.expect("migrate after repair");
    assert_eq!(reader.entries().await, ["a", "b", "a2"]);

    // 6. Two instances racing on a changed repeatable run it exactly once.
    write("R__a.sql", &log_entry("a4"));
    let (x, y) = tokio::join!(migrate(), migrate());
    x.expect("instance A");
    y.expect("instance B");
    assert_eq!(reader.entries().await, ["a", "b", "a2", "a4"]);

    // 7. A failing repeatable is recorded failed, rolls back, blocks, and `repair` clears it.
    write("R__bad.sql", "insert into missing_table values (1);");
    let err = migrate().await.unwrap_err();
    assert!(
        matches!(err, SchemaInstallerError::MigrationFailed { ref version, .. } if version == "R__bad"),
        "{err:?}"
    );
    let rows = applied(config).await;
    assert_eq!(
        rows.iter().find(|r| r.name == "bad").unwrap().status,
        "failed"
    );
    assert!(
        Migrator::validate(config, source(dir)).await.is_err(),
        "a failed repeatable is a validation issue"
    );
    write("R__bad.sql", &log_entry("fixed"));
    assert!(migrate().await.is_err(), "failed row blocks until repair");
    Migrator::repair(config, source(dir)).await.expect("repair");
    migrate().await.expect("migrate after fix");
    assert_eq!(reader.entries().await.last().unwrap(), "fixed");

    // 8. An applied repeatable whose file was removed is reported.
    std::fs::remove_file(dir.join("R__b.sql")).unwrap();
    let err = migrate().await.unwrap_err();
    assert!(
        matches!(err, SchemaInstallerError::MissingMigrationSource { ref version, .. } if version == "R__b"),
        "{err:?}"
    );
}
