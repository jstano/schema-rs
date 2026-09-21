use crate::error::SchemaInstallerError;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// The fixed, reserved version of the legacy XML `install` command (see `installer.rs`)
/// records its single tracking row under, so it can never collide with real migration
/// versions (which start at V1+). It never corresponds to an actual migration file, so
/// callers that cross-reference `schema_migration` against a `MigrationSource` (e.g.
/// `Migrator::validate`) must treat it as exempt rather than "missing."
pub(crate) const RESERVED_INSTALL_VERSION: &str = "0";

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: String,
    pub description: String,
    pub script_path: String,
    pub sql: String,
}

#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub id: i64,
    pub version: String,
    pub script_path: String,
    pub checksum: String,
    pub execution_time_ms: i64,
    pub installed_at: String,
    pub status: String,
    pub tool_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    Success,
    Failed,
    Pending,
}

impl MigrationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MigrationStatus::Success => "success",
            MigrationStatus::Failed => "failed",
            MigrationStatus::Pending => "pending",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(MigrationStatus::Success),
            "failed" => Some(MigrationStatus::Failed),
            "pending" => Some(MigrationStatus::Pending),
            _ => None,
        }
    }
}

pub trait MigrationSource: Send + Sync {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError>;
}

pub struct DirectoryMigrationSource {
    pub path: PathBuf,
}

impl MigrationSource for DirectoryMigrationSource {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError> {
        let mut migrations = Vec::new();

        if !self.path.exists() {
            return Err(SchemaInstallerError::InvalidConfiguration(
                format!("Migrations directory does not exist: {:?}", self.path),
            ));
        }

        if !self.path.is_dir() {
            return Err(SchemaInstallerError::InvalidConfiguration(
                format!("Migrations path is not a directory: {:?}", self.path),
            ));
        }

        let entries = std::fs::read_dir(&self.path)
            .map_err(SchemaInstallerError::Io)?;

        for entry in entries {
            let entry = entry.map_err(SchemaInstallerError::Io)?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let filename = path
                .file_name()
                .and_then(|f| f.to_str())
                .ok_or_else(|| {
                    SchemaInstallerError::InvalidConfiguration(
                        "Invalid filename encoding".to_string(),
                    )
                })?;

            if !filename.to_lowercase().ends_with(".sql") {
                continue;
            }

            let (version, description) = match parse_migration_filename(filename)? {
                Some(parsed) => parsed,
                None => continue,
            };
            let sql = std::fs::read_to_string(&path)
                .map_err(SchemaInstallerError::Io)?;

            let script_path = path.to_string_lossy().to_string();

            migrations.push(Migration {
                version,
                description,
                script_path,
                sql,
            });
        }

        migrations.sort_by(|a, b| compare_versions(&a.version, &b.version));

        check_no_duplicate_versions(&migrations)?;

        Ok(migrations)
    }
}

pub struct EmbeddedMigrationSource {
    pub migrations: Vec<Migration>,
}

impl MigrationSource for EmbeddedMigrationSource {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError> {
        check_no_duplicate_versions(&self.migrations)?;

        Ok(self.migrations.clone())
    }
}

/// Hard-errors on duplicate migration versions, matching Flyway's behavior. Without this,
/// two files resolving to the same version (e.g. `V1__add_users.sql` and
/// `V1__add_orders.sql`) would silently let the first one `read_dir` happens to return win
/// the slot; the second would hit the `UNIQUE (version)` constraint on insert, get
/// misread as `ConcurrentMigrationDetected`, see the first's `status == "success"` and be
/// reported as "already applied by another process; skipping" - never executed and never
/// recorded, with the winner depending on filesystem iteration order (H13).
fn check_no_duplicate_versions(migrations: &[Migration]) -> Result<(), SchemaInstallerError> {
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();

    for migration in migrations {
        if let Some(first_path) = seen.insert(migration.version.as_str(), migration.script_path.as_str()) {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "Duplicate migration version '{}': {} and {}",
                migration.version, first_path, migration.script_path
            )));
        }
    }

    Ok(())
}

/// Parses a `V{version}__{description}.sql`-style filename; the `__{description}` part
/// is optional, so `V{version}.sql` is also valid and parses with an empty description.
/// Returns `Ok(None)` for a file that isn't a versioned migration at all (its name doesn't
/// start with `V`/`v`) -
/// e.g. a Flyway-style repeatable (`R__...`) or undo (`U__...`) migration, or any other
/// file someone dropped into the directory - so the caller can skip it with a warning
/// instead of aborting the entire directory scan over a file this tool was never going
/// to manage (H11). A file that *does* start with `V` but is otherwise malformed still
/// hard-errors, since that shape is far more likely to be a typo in a real migration
/// than an intentional non-versioned file.
fn parse_migration_filename(filename: &str) -> Result<Option<(String, String)>, SchemaInstallerError> {
    let name_without_ext = filename
        .strip_suffix(".sql")
        .ok_or_else(|| {
            SchemaInstallerError::InvalidConfiguration(
                format!("File does not end with .sql: {}", filename),
            )
        })?;

    if !name_without_ext.to_lowercase().starts_with('v') {
        eprintln!(
            "Warning: skipping non-versioned migration file (expected V{{version}}__{{description}}.sql): {}",
            filename
        );
        return Ok(None);
    }

    // The `__description` suffix is optional - a filename with no `__` at all (e.g.
    // `V20240115143022.sql`, as produced by schema-migration-generator's
    // --auto-generate-name with no --description) is equivalent to an empty description,
    // matching the already-legal `V20240115143022__.sql` form.
    let parts: Vec<&str> = name_without_ext.splitn(2, "__").collect();

    let version_part = parts[0].to_lowercase();
    let version = version_part[1..].to_string();
    let description = parts.get(1).map(|d| d.replace('_', " ")).unwrap_or_default();

    if version.is_empty() {
        return Err(SchemaInstallerError::InvalidConfiguration(
            format!("Migration version cannot be empty: {}", filename),
        ));
    }

    // Reject anything that isn't a plain dot/underscore-separated run of numbers (e.g.
    // "1", "1.2", "1_2"). Without this, a typo like `Vfinal__x.sql` or `V1a__x.sql`
    // would parse "successfully" into a version whose non-numeric segments
    // `compare_versions` then silently drops, sorting it before every real version
    // instead of failing fast with a clear error.
    if !version.split(['.', '_']).all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())) {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Migration version must be a dot/underscore-separated list of numbers (e.g. 'V1', 'V1.2', 'V1_2'), got '{}': {}",
            version, filename
        )));
    }

    // Version "0" is reserved for the legacy XML `install` command's tracking row (see
    // RESERVED_INSTALL_VERSION) and never corresponds to a real migration file. A user
    // migration named `V0__...sql` would collide with it, producing a confusing
    // checksum-mismatch error instead of a clear "this version is reserved" one.
    if version == RESERVED_INSTALL_VERSION {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Migration version '0' is reserved for the legacy install command and cannot be used by a migration file: {}",
            filename
        )));
    }

    Ok(Some((version, description)))
}

pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    // Version strings may use either `.` (e.g. "1.2") or `_` (e.g. "1_2", produced by a
    // filename like `V1_2__add_email_column.sql`) as the separator, so both must be
    // split on here, or multipart underscore versions silently compare as equal.
    let split = |v: &str| -> Vec<u64> {
        v.split(['.', '_'])
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };
    let v1_parts = split(v1);
    let v2_parts = split(v2);

    for (p1, p2) in v1_parts.iter().zip(v2_parts.iter()) {
        if p1 != p2 {
            return p1.cmp(p2);
        }
    }

    v1_parts.len().cmp(&v2_parts.len())
}

pub fn compute_checksum(sql: &str) -> String {
    // Normalize all three line-ending styles to `\n`: Windows (`\r\n`), Unix (`\n`,
    // already a no-op), and old-Mac-style bare `\r` - normalizing only `\r\n` left a
    // migration file saved with lone `\r` line endings hashing differently from the
    // same logical content saved with `\n`, causing a spurious `ChecksumMismatch` for a
    // purely cosmetic line-ending change.
    let normalized = sql.trim().replace("\r\n", "\n").replace('\r', "\n");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_migration_filename() {
        let (version, description) = parse_migration_filename("V1__create_users.sql").unwrap().unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "create users");

        let (version, description) = parse_migration_filename("V1_2__add_email_column.sql").unwrap().unwrap();
        assert_eq!(version, "1_2");
        assert_eq!(description, "add email column");
    }

    #[test]
    fn test_parse_migration_filename_without_description() {
        // The `__description` suffix is optional (e.g. produced by
        // schema-migration-generator's --auto-generate-name with no --description).
        let (version, description) = parse_migration_filename("V20240115143022.sql").unwrap().unwrap();
        assert_eq!(version, "20240115143022");
        assert_eq!(description, "");

        // Already-legal today, and must keep working the same way: an explicit `__`
        // with nothing after it is likewise an empty description.
        let (version, description) = parse_migration_filename("V1__.sql").unwrap().unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "");
    }

    #[test]
    fn test_parse_migration_filename_case_insensitive() {
        let (version, description) = parse_migration_filename("v1__create_users.sql").unwrap().unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "create users");
    }

    #[test]
    fn test_parse_migration_filename_skips_non_versioned_files() {
        // Flyway-style repeatable (`R__...`) and undo (`U__...`) migrations, and any
        // other file that doesn't start with `V`, aren't versioned migrations this tool
        // manages - they must be skipped (Ok(None)) rather than aborting the whole
        // directory scan (H11).
        assert!(parse_migration_filename("R__refresh_view.sql").unwrap().is_none());
        assert!(parse_migration_filename("U__undo_something.sql").unwrap().is_none());
        assert!(parse_migration_filename("readme.sql").unwrap().is_none());
    }

    #[test]
    fn test_parse_migration_filename_rejects_reserved_install_version() {
        // Version "0" is reserved for the legacy XML `install` command's tracking row;
        // a user migration claiming it would collide and produce a confusing
        // checksum-mismatch error instead of a clear one at parse time.
        let err = parse_migration_filename("V0__do_something.sql").unwrap_err();
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_parse_migration_filename_rejects_non_numeric_version_segments() {
        // A typo'd version (e.g. "final" instead of a number) used to parse
        // "successfully" into a version whose non-numeric segments compare_versions
        // then silently dropped, sorting it before every real version.
        let err = parse_migration_filename("Vfinal__do_something.sql").unwrap_err();
        assert!(err.to_string().contains("must be a dot/underscore-separated list of numbers"));

        let err = parse_migration_filename("V1a__do_something.sql").unwrap_err();
        assert!(err.to_string().contains("must be a dot/underscore-separated list of numbers"));
    }

    #[test]
    fn test_version_comparison() {
        assert!(compare_versions("1", "2") == std::cmp::Ordering::Less);
        assert!(compare_versions("2", "1") == std::cmp::Ordering::Greater);
        assert!(compare_versions("1", "1") == std::cmp::Ordering::Equal);
        assert!(compare_versions("1.2", "1.3") == std::cmp::Ordering::Less);
        assert!(compare_versions("1.10", "1.2") == std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_version_comparison_underscore_separated() {
        // Regression test: version parts produced by `V1_2__desc.sql`-style filenames
        // must compare correctly, not silently reduce to empty (equal) part vectors.
        assert!(compare_versions("1_2", "1_3") == std::cmp::Ordering::Less);
        assert!(compare_versions("1_10", "1_2") == std::cmp::Ordering::Greater);
        assert!(compare_versions("1_2", "1_2") == std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_check_no_duplicate_versions_ok() {
        let migrations = vec![
            Migration {
                version: "1".to_string(),
                description: "add users".to_string(),
                script_path: "V1__add_users.sql".to_string(),
                sql: String::new(),
            },
            Migration {
                version: "2".to_string(),
                description: "add orders".to_string(),
                script_path: "V2__add_orders.sql".to_string(),
                sql: String::new(),
            },
        ];

        assert!(check_no_duplicate_versions(&migrations).is_ok());
    }

    #[test]
    fn test_check_no_duplicate_versions_rejects_duplicates() {
        // Regression test for H13: two migration files resolving to the same version
        // must hard-error at load time rather than let the first one `read_dir` happens
        // to return silently win the slot while the second is never executed.
        let migrations = vec![
            Migration {
                version: "1".to_string(),
                description: "add users".to_string(),
                script_path: "V1__add_users.sql".to_string(),
                sql: String::new(),
            },
            Migration {
                version: "1".to_string(),
                description: "add orders".to_string(),
                script_path: "V1__add_orders.sql".to_string(),
                sql: String::new(),
            },
        ];

        let err = check_no_duplicate_versions(&migrations).unwrap_err();
        assert!(err.to_string().contains("Duplicate migration version '1'"));
        assert!(err.to_string().contains("V1__add_users.sql"));
        assert!(err.to_string().contains("V1__add_orders.sql"));
    }

    #[test]
    fn test_directory_migration_source_rejects_duplicate_versions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("V1__add_users.sql"), "CREATE TABLE users (id INT);").unwrap();
        std::fs::write(dir.path().join("V1__add_orders.sql"), "CREATE TABLE orders (id INT);").unwrap();

        let source = DirectoryMigrationSource { path: dir.path().to_path_buf() };
        let err = source.migrations().unwrap_err();
        assert!(err.to_string().contains("Duplicate migration version '1'"));
    }

    #[test]
    fn test_compute_checksum() {
        let sql = "CREATE TABLE users (id BIGSERIAL PRIMARY KEY);";
        let checksum1 = compute_checksum(sql);
        let checksum2 = compute_checksum(sql);
        assert_eq!(checksum1, checksum2);

        let checksum3 = compute_checksum("CREATE TABLE posts (id BIGSERIAL PRIMARY KEY);");
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_compute_checksum_normalizes_whitespace() {
        let sql1 = "CREATE TABLE users (id BIGSERIAL PRIMARY KEY);";
        let sql2 = "CREATE TABLE users (id BIGSERIAL PRIMARY KEY);\n";
        let sql3 = "CREATE TABLE users (\n  id BIGSERIAL PRIMARY KEY\n);";

        let checksum1 = compute_checksum(sql1);
        let checksum2 = compute_checksum(sql2);
        let checksum3 = compute_checksum(sql3);

        assert_eq!(checksum1, checksum2);
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_compute_checksum_normalizes_bare_carriage_return_line_endings() {
        // Regression test: a migration file edited/saved with old-Mac-style bare `\r`
        // line endings must hash the same as the identical content saved with `\n`
        // (or `\r\n`), not produce a spurious checksum mismatch for a cosmetic change.
        let unix = "CREATE TABLE users (\n  id BIGSERIAL PRIMARY KEY\n);";
        let windows = "CREATE TABLE users (\r\n  id BIGSERIAL PRIMARY KEY\r\n);";
        let old_mac = "CREATE TABLE users (\r  id BIGSERIAL PRIMARY KEY\r);";

        let checksum_unix = compute_checksum(unix);
        let checksum_windows = compute_checksum(windows);
        let checksum_old_mac = compute_checksum(old_mac);

        assert_eq!(checksum_unix, checksum_windows);
        assert_eq!(checksum_unix, checksum_old_mac);
    }
}
