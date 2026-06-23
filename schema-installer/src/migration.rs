use crate::error::SchemaInstallerError;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

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
            .map_err(|e| SchemaInstallerError::Io(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| SchemaInstallerError::Io(e))?;
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
                .map_err(|e| SchemaInstallerError::Io(e))?;

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

    Ok((version, description))
}

pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let v1_parts: Vec<u64> = v1
        .split('.')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect();
    let v2_parts: Vec<u64> = v2
        .split('.')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect();

    for (p1, p2) in v1_parts.iter().zip(v2_parts.iter()) {
        if p1 != p2 {
            return p1.cmp(p2);
        }
    }

    v1_parts.len().cmp(&v2_parts.len())
}

pub fn compute_checksum(sql: &str) -> String {
    let normalized = sql.trim().replace("\r\n", "\n");
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
    fn test_version_comparison() {
        assert!(compare_versions("1", "2") == std::cmp::Ordering::Less);
        assert!(compare_versions("2", "1") == std::cmp::Ordering::Greater);
        assert!(compare_versions("1", "1") == std::cmp::Ordering::Equal);
        assert!(compare_versions("1.2", "1.3") == std::cmp::Ordering::Less);
        assert!(compare_versions("1.10", "1.2") == std::cmp::Ordering::Greater);
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
}
