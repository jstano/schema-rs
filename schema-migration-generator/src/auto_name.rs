use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

/// Builds a Flyway-style `V{version}__{description}.sql` filename from a timestamp and
/// an optional description. The description is omitted entirely (rather than left as a
/// trailing empty `__`) when not given.
pub fn generate_migration_filename(timestamp: DateTime<Utc>, description: Option<&str>) -> String {
    let version = timestamp.format("%Y%m%d%H%M%S");
    match description {
        Some(desc) if !desc.is_empty() => format!("V{version}__{}.sql", desc.replace(' ', "_")),
        _ => format!("V{version}.sql"),
    }
}

/// Builds the full output path for an auto-generated migration: the generated filename,
/// placed in `output_dir` when given, or alongside `schema_file` otherwise (when there is
/// one - `--old`/`--new` mode has no single schema file to anchor to, so it falls back to
/// the current directory unless `--output-dir` is given).
pub fn generate_migration_path(
    schema_file: Option<&str>,
    output_dir: Option<&str>,
    timestamp: DateTime<Utc>,
    description: Option<&str>,
) -> PathBuf {
    let dir = match (output_dir, schema_file) {
        (Some(dir), _) => Path::new(dir).to_path_buf(),
        (None, Some(schema_file)) => Path::new(schema_file).parent().unwrap_or_else(|| Path::new(".")).to_path_buf(),
        (None, None) => Path::new(".").to_path_buf(),
    };
    dir.join(generate_migration_filename(timestamp, description))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 22).unwrap()
    }

    #[test]
    fn generates_filename_without_description() {
        assert_eq!(generate_migration_filename(sample_timestamp(), None), "V20240115143022.sql");
        assert_eq!(generate_migration_filename(sample_timestamp(), Some("")), "V20240115143022.sql");
    }

    #[test]
    fn generates_filename_with_description() {
        assert_eq!(
            generate_migration_filename(sample_timestamp(), Some("add users table")),
            "V20240115143022__add_users_table.sql"
        );
    }

    #[test]
    fn derives_directory_from_schema_file_path() {
        let path = generate_migration_path(Some("migrations/schema.xml"), None, sample_timestamp(), None);
        assert_eq!(path, PathBuf::from("migrations/V20240115143022.sql"));

        let path = generate_migration_path(Some("schema.xml"), None, sample_timestamp(), Some("add users table"));
        assert_eq!(path, PathBuf::from("V20240115143022__add_users_table.sql"));
    }

    #[test]
    fn output_dir_overrides_the_schema_files_directory() {
        let path = generate_migration_path(
            Some("migrations/schema.xml"),
            Some("build/migrations"),
            sample_timestamp(),
            Some("add users table"),
        );
        assert_eq!(path, PathBuf::from("build/migrations/V20240115143022__add_users_table.sql"));
    }

    #[test]
    fn defaults_to_current_directory_when_no_schema_file_or_output_dir() {
        let path = generate_migration_path(None, None, sample_timestamp(), None);
        assert_eq!(path, PathBuf::from("./V20240115143022.sql"));
    }

    #[test]
    fn output_dir_is_used_when_there_is_no_schema_file() {
        let path = generate_migration_path(None, Some("build/migrations"), sample_timestamp(), Some("add users table"));
        assert_eq!(path, PathBuf::from("build/migrations/V20240115143022__add_users_table.sql"));
    }
}
