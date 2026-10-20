//! End-to-end tests for repeatable (`R__`) migrations against SQLite.

use schema_installer::connection::AnyPool;
use schema_installer::{
    DirectoryMigrationSource, EmbeddedMigrationSource, Migration, Migrator, RepeatableMigration,
    SchemaInstallerConfig, SchemaInstallerConfigBuilder, SchemaInstallerError,
};
use schema_sql_generator::common::generator_type::GeneratorType;
use sqlx::Row;
use tempfile::TempDir;

struct Env {
    _temp: TempDir,
    migrations: std::path::PathBuf,
    connection_string: String,
    config: SchemaInstallerConfig,
}

impl Env {
    /// A directory holding `V1__create_log.sql` (creates the `log` table) and a database.
    fn new() -> Self {
        let temp = TempDir::new().expect("temp dir");
        let migrations = temp.path().join("migrations");
        std::fs::create_dir(&migrations).unwrap();
        std::fs::write(
            migrations.join("V1__create_log.sql"),
            "create table log (id integer primary key autoincrement, entry text not null);",
        )
        .unwrap();
        let connection_string = format!("sqlite://{}", temp.path().join("test.db").display());
        let config = SchemaInstallerConfigBuilder::new()
            .database_type(GeneratorType::Sqlite)
            .connection_string(connection_string.clone())
            .build()
            .expect("valid config");
        Env {
            _temp: temp,
            migrations,
            connection_string,
            config,
        }
    }

    fn write(&self, name: &str, sql: &str) {
        std::fs::write(self.migrations.join(name), sql).unwrap();
    }

    fn source(&self) -> Box<DirectoryMigrationSource> {
        Box::new(DirectoryMigrationSource {
            path: self.migrations.clone(),
        })
    }

    async fn migrate(&self) -> Result<(), SchemaInstallerError> {
        Migrator::migrate(&self.config, self.source()).await
    }

    async fn pool(&self) -> sqlx::SqlitePool {
        sqlx::sqlite::SqlitePoolOptions::new()
            .connect(&self.connection_string)
            .await
            .expect("connect")
    }

    async fn log(&self) -> Vec<String> {
        sqlx::query("select entry from log order by id")
            .fetch_all(&self.pool().await)
            .await
            .expect("read log")
            .iter()
            .map(|row| row.get::<String, _>("entry"))
            .collect()
    }

    async fn applied(&self) -> Vec<schema_installer::AppliedRepeatableMigration> {
        let pool = AnyPool::connect(&GeneratorType::Sqlite, &self.connection_string, false)
            .await
            .unwrap();
        pool.get_applied_repeatables().await.unwrap()
    }
}

fn log_entry(entry: &str) -> String {
    format!("insert into log (entry) values ('{entry}');")
}

#[tokio::test]
async fn repeatables_run_after_versioned_migrations_in_name_order() {
    let env = Env::new();
    // Written out of order on purpose; both depend on the table V1 creates.
    env.write("R__b_second.sql", &log_entry("b"));
    env.write("R__a_first.sql", &log_entry("a"));

    env.migrate().await.expect("migrate");

    assert_eq!(env.log().await, ["a", "b"]);
    let applied = env.applied().await;
    assert_eq!(applied.len(), 2);
    assert!(applied.iter().all(|r| r.status == "success"));
}

#[tokio::test]
async fn unchanged_repeatable_is_not_rerun() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));

    env.migrate().await.unwrap();
    env.migrate().await.unwrap();

    assert_eq!(env.log().await, ["a"]);
}

#[tokio::test]
async fn changed_repeatable_is_rerun_and_tracked_in_place() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));
    env.migrate().await.unwrap();
    let before = env.applied().await;

    env.write("R__a.sql", &log_entry("a2"));
    env.migrate().await.unwrap();

    assert_eq!(env.log().await, ["a", "a2"]);
    let after = env.applied().await;
    assert_eq!(after.len(), 1, "one row per repeatable, updated in place");
    assert_eq!(after[0].id, before[0].id);
    assert_ne!(after[0].checksum, before[0].checksum);
    assert_eq!(after[0].status, "success");
}

#[tokio::test]
async fn failed_repeatable_blocks_until_cleared() {
    let env = Env::new();
    env.write("R__bad.sql", "insert into missing_table values (1);");

    let err = env.migrate().await.unwrap_err();
    assert!(
        matches!(err, SchemaInstallerError::MigrationFailed { ref version, .. } if version == "R__bad"),
        "{err:?}"
    );
    assert_eq!(env.applied().await[0].status, "failed");

    // Fixing the script is not enough on its own: the failed row blocks, like a versioned one.
    env.write("R__bad.sql", &log_entry("fixed"));
    let err = env.migrate().await.unwrap_err();
    assert!(err.to_string().contains("repair"), "{err}");

    let pool = AnyPool::connect(&GeneratorType::Sqlite, &env.connection_string, false)
        .await
        .unwrap();
    assert_eq!(pool.delete_failed_repeatables().await.unwrap(), 1);
    env.migrate().await.unwrap();
    assert_eq!(env.log().await, ["fixed"]);
}

#[tokio::test]
async fn target_skips_repeatables() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));

    Migrator::migrate_to_target(&env.config, env.source(), Some("1"))
        .await
        .unwrap();
    assert!(env.log().await.is_empty());
    assert!(env.applied().await.is_empty());

    env.migrate().await.unwrap();
    assert_eq!(env.log().await, ["a"]);
}

#[tokio::test]
async fn dry_run_lists_repeatables_without_applying_them() {
    let env = Env::new();
    env.write("R__refresh_things.sql", &log_entry("a"));
    let output = env._temp.path().join("dry-run.sql");

    Migrator::dry_run(&env.config, env.source(), None, &output)
        .await
        .unwrap();

    let contents = std::fs::read_to_string(&output).unwrap();
    assert!(contents.contains("-- Repeatable migration refresh things (R__refresh_things.sql)"));
    assert!(contents.contains("insert into log"));
    assert!(contents.find("create table log") < contents.find("insert into log"));
    assert!(env.applied().await.is_empty());
}

#[tokio::test]
async fn concurrent_migrate_runs_a_changed_repeatable_once() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));
    env.migrate().await.unwrap();
    env.write("R__a.sql", &log_entry("a2"));

    let (a, b) = tokio::join!(env.migrate(), env.migrate());
    a.expect("instance A");
    b.expect("instance B");

    assert_eq!(env.log().await, ["a", "a2"]);
}

#[tokio::test]
async fn applied_repeatable_whose_file_was_removed_is_reported() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));
    env.migrate().await.unwrap();

    std::fs::remove_file(env.migrations.join("R__a.sql")).unwrap();
    let err = env.migrate().await.unwrap_err();
    assert!(
        matches!(err, SchemaInstallerError::MissingMigrationSource { ref version, .. } if version == "R__a"),
        "{err:?}"
    );
}

#[tokio::test]
async fn repeatable_with_its_own_transaction_control_needs_the_conf_opt_out() {
    let env = Env::new();
    env.write(
        "R__tx.sql",
        &format!("begin;\n{}\ncommit;", log_entry("tx")),
    );

    let err = env.migrate().await.unwrap_err();
    assert!(matches!(err, SchemaInstallerError::InvalidConfiguration(_)));
    assert!(err.to_string().contains("R__tx.sql.conf"), "{err}");

    env.write("R__tx.sql.conf", "executeInTransaction=false\n");
    env.migrate().await.unwrap();
    assert_eq!(env.log().await, ["tx"]);
}

impl Env {
    async fn has_pending(&self) -> bool {
        Migrator::has_pending_migrations(&self.config, self.source())
            .await
            .expect("has_pending_migrations")
    }
}

#[tokio::test]
async fn pending_check_reflects_new_applied_and_changed_repeatables() {
    let env = Env::new();
    env.migrate().await.unwrap();
    assert!(!env.has_pending().await);

    env.write("R__a.sql", &log_entry("a"));
    assert!(env.has_pending().await, "a new repeatable is pending");

    env.migrate().await.unwrap();
    assert!(!env.has_pending().await, "applied and unchanged");

    env.write("R__a.sql", &log_entry("a2"));
    assert!(env.has_pending().await, "a changed repeatable is pending");
}

#[tokio::test]
async fn validate_accepts_a_changed_repeatable_but_flags_failed_and_missing() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));
    env.migrate().await.unwrap();
    Migrator::validate(&env.config, env.source())
        .await
        .expect("applied and unchanged is valid");

    // Changed: pending, not an error.
    env.write("R__a.sql", &log_entry("a2"));
    Migrator::validate(&env.config, env.source())
        .await
        .expect("a changed repeatable is not a validation error");

    // Missing source file.
    std::fs::remove_file(env.migrations.join("R__a.sql")).unwrap();
    let err = Migrator::validate(&env.config, env.source())
        .await
        .unwrap_err();
    assert!(
        matches!(err, SchemaInstallerError::ValidationFailed(_)),
        "{err:?}"
    );

    // Failed run.
    env.write("R__a.sql", &log_entry("a"));
    env.write("R__bad.sql", "insert into missing_table values (1);");
    env.migrate().await.unwrap_err();
    let err = Migrator::validate(&env.config, env.source())
        .await
        .unwrap_err();
    assert!(
        matches!(err, SchemaInstallerError::ValidationFailed(_)),
        "{err:?}"
    );
}

#[tokio::test]
async fn repair_clears_failed_repeatables_so_a_fixed_script_runs() {
    let env = Env::new();
    env.write("R__bad.sql", "insert into missing_table values (1);");
    env.migrate().await.unwrap_err();

    env.write("R__bad.sql", &log_entry("fixed"));
    Migrator::repair(&env.config, env.source())
        .await
        .expect("repair");
    assert!(env.applied().await.is_empty());

    env.migrate().await.unwrap();
    assert_eq!(env.log().await, ["fixed"]);
}

#[tokio::test]
async fn repair_realigns_a_changed_repeatables_checksum_without_rerunning_it() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));
    env.migrate().await.unwrap();

    env.write("R__a.sql", &log_entry("a-edited"));
    Migrator::repair(&env.config, env.source())
        .await
        .expect("repair");

    assert!(!env.has_pending().await, "the edit was accepted by repair");
    env.migrate().await.unwrap();
    assert_eq!(env.log().await, ["a"], "the edited script did not run");
}

#[tokio::test]
async fn info_runs_with_applied_changed_and_pending_repeatables() {
    let env = Env::new();
    env.write("R__a.sql", &log_entry("a"));
    env.migrate().await.unwrap();
    env.write("R__a.sql", &log_entry("a2"));
    env.write("R__b.sql", &log_entry("b"));

    Migrator::info(&env.config, env.source())
        .await
        .expect("info");
}

#[tokio::test]
async fn embedded_source_applies_and_reruns_repeatables() {
    let env = Env::new();
    let source = |view_sql: &str| {
        Box::new(EmbeddedMigrationSource {
            migrations: vec![Migration {
                version: "1".to_string(),
                description: "create log".to_string(),
                script_path: "V1__create_log.sql".to_string(),
                sql:
                    "create table log (id integer primary key autoincrement, entry text not null);"
                        .to_string(),
                transactional: true,
            }],
            repeatables: vec![
                // Deliberately out of name order.
                RepeatableMigration {
                    name: "b".to_string(),
                    script_path: "R__b.sql".to_string(),
                    sql: log_entry("b"),
                    transactional: true,
                },
                RepeatableMigration {
                    name: "a".to_string(),
                    script_path: "R__a.sql".to_string(),
                    sql: view_sql.to_string(),
                    transactional: true,
                },
            ],
        })
    };

    Migrator::migrate(&env.config, source(&log_entry("a")))
        .await
        .unwrap();
    assert_eq!(env.log().await, ["a", "b"]);

    // Unchanged: no-op. Changed: only that repeatable re-runs.
    Migrator::migrate(&env.config, source(&log_entry("a")))
        .await
        .unwrap();
    assert_eq!(env.log().await, ["a", "b"]);
    Migrator::migrate(&env.config, source(&log_entry("a2")))
        .await
        .unwrap();
    assert_eq!(env.log().await, ["a", "b", "a2"]);
}
