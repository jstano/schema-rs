use crate::error::SchemaInstallerError;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// The fixed, reserved version of the legacy XML `install` command (see `installer.rs`)
/// records its single tracking row under, so it can never collide with real migration
/// versions (which start at V1+). It never corresponds to an actual migration file, so
/// callers that cross-reference `schema_migration` against a `MigrationSource` (e.g.
/// `Migrator::validate`) must treat it as exempt rather than "missing."
pub(crate) const RESERVED_INSTALL_VERSION: &str = "0";

#[derive(Clone)]
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

            let (version, description) = parse_migration_filename(filename)?;
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

        Ok(migrations)
    }
}

pub struct EmbeddedMigrationSource {
    pub migrations: Vec<Migration>,
}

impl MigrationSource for EmbeddedMigrationSource {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError> {
        Ok(self.migrations.clone())
    }
}

fn parse_migration_filename(filename: &str) -> Result<(String, String), SchemaInstallerError> {
    let name_without_ext = filename
        .strip_suffix(".sql")
        .ok_or_else(|| {
            SchemaInstallerError::InvalidConfiguration(
                format!("File does not end with .sql: {}", filename),
            )
        })?;

    let parts: Vec<&str> = name_without_ext.splitn(2, "__").collect();

    if parts.len() != 2 {
        return Err(SchemaInstallerError::InvalidConfiguration(
            format!(
                "Invalid migration filename format (expected V{{version}}__{{description}}.sql): {}",
                filename
            ),
        ));
    }

    let version_part = parts[0].to_lowercase();
    if !version_part.starts_with('v') {
        return Err(SchemaInstallerError::InvalidConfiguration(
            format!(
                "Migration filename must start with V (case-insensitive): {}",
                filename
            ),
        ));
    }

    let version = version_part[1..].to_string();
    let description = parts[1].replace('_', " ");

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

    Ok((version, description))
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
        let (version, description) = parse_migration_filename("V1__create_users.sql").unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "create users");

        let (version, description) = parse_migration_filename("V1_2__add_email_column.sql").unwrap();
        assert_eq!(version, "1_2");
        assert_eq!(description, "add email column");
    }

    #[test]
    fn test_parse_migration_filename_case_insensitive() {
        let (version, description) = parse_migration_filename("v1__create_users.sql").unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "create users");
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
