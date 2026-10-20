use crate::error::SchemaInstallerError;
use schema_sql_generator::common::generator_type::GeneratorType;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
    /// `false` runs each statement in autocommit mode, with no wrapping transaction, so
    /// statements that refuse to run inside one (`CREATE INDEX CONCURRENTLY`, `VACUUM`,
    /// `ALTER TYPE ... ADD VALUE`) can be migrated (M18). Loaded from a sibling
    /// `<script>.conf` file by `DirectoryMigrationSource`; always `true` unless set.
    pub transactional: bool,
}

/// A Flyway-style repeatable migration (`R__{description}.sql`). It has no version: it is
/// identified by `name` and re-applied whenever its checksum changes, after all versioned
/// migrations. Tracked in its own table, separately from [`Migration`] (see
/// REPEATABLE_MIGRATIONS_PLAN.md).
#[derive(Debug, Clone)]
pub struct RepeatableMigration {
    /// The part of the filename between `R__` and `.sql`, exactly as written. This is the
    /// unique key the migration is tracked under.
    pub name: String,
    pub script_path: String,
    pub sql: String,
    /// Same meaning as [`Migration::transactional`].
    pub transactional: bool,
}

impl RepeatableMigration {
    /// Human-readable form of `name` (underscores shown as spaces), mirroring how
    /// versioned migration descriptions are derived.
    pub fn description(&self) -> String {
        self.name.replace('_', " ")
    }
}

#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub id: i64,
    pub version: String,
    pub script_path: String,
    pub description: String,
    pub checksum: String,
    pub execution_time_ms: i64,
    pub installed_at: String,
    pub status: String,
    pub tool_version: String,
}

/// A row of `schema_repeatable_migration`: the last recorded run of a repeatable migration.
#[derive(Debug, Clone)]
pub struct AppliedRepeatableMigration {
    pub id: i64,
    pub name: String,
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
}

impl std::str::FromStr for MigrationStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "success" => Ok(MigrationStatus::Success),
            "failed" => Ok(MigrationStatus::Failed),
            "pending" => Ok(MigrationStatus::Pending),
            _ => Err(()),
        }
    }
}

pub trait MigrationSource: Send + Sync {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError>;

    /// Repeatable (`R__`) migrations, sorted by name. Sources that don't support them
    /// return none.
    fn repeatable_migrations(&self) -> Result<Vec<RepeatableMigration>, SchemaInstallerError> {
        Ok(Vec::new())
    }
}

pub struct DirectoryMigrationSource {
    pub path: PathBuf,
}

/// `.sql` files as `(filename, path)`, and subdirectories, found by a directory scan.
type DirectoryScan = (Vec<(String, PathBuf)>, Vec<PathBuf>);

impl DirectoryMigrationSource {
    /// Validates the directory and returns its regular `.sql` files as `(filename, path)`,
    /// plus any subdirectories (which are never scanned).
    fn scan(&self) -> Result<DirectoryScan, SchemaInstallerError> {
        if !self.path.exists() {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "Migrations directory does not exist: {:?}",
                self.path
            )));
        }

        if !self.path.is_dir() {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "Migrations path is not a directory: {:?}",
                self.path
            )));
        }

        let mut files = Vec::new();
        let mut subdirs = Vec::new();

        for entry in std::fs::read_dir(&self.path).map_err(SchemaInstallerError::Io)? {
            let path = entry.map_err(SchemaInstallerError::Io)?.path();

            if path.is_dir() {
                subdirs.push(path);
                continue;
            }

            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|f| f.to_str()).ok_or_else(|| {
                SchemaInstallerError::InvalidConfiguration("Invalid filename encoding".to_string())
            })?;

            if !filename.to_lowercase().ends_with(".sql") {
                continue;
            }

            files.push((filename.to_string(), path));
        }

        Ok((files, subdirs))
    }
}

impl MigrationSource for DirectoryMigrationSource {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError> {
        let mut migrations = Vec::new();
        let (files, subdirs) = self.scan()?;

        for subdir in subdirs {
            // Deliberately flat (L13): one directory sorted by version shows the whole
            // history at a glance. Warn rather than silently ignore nested migrations.
            eprintln!(
                "Warning: skipping subdirectory {} (migration subdirectories are not scanned)",
                subdir.display()
            );
        }

        for (filename, path) in files {
            // Repeatable migrations are returned by `repeatable_migrations`.
            if is_repeatable_filename(&filename) {
                continue;
            }

            let (version, description) = match parse_migration_filename(&filename)? {
                Some(parsed) => parsed,
                None => continue,
            };
            let sql = std::fs::read_to_string(&path).map_err(SchemaInstallerError::Io)?;
            let transactional = read_migration_config(&path)?;

            // Just the filename, not the absolute path: the path differs between dev, CI and
            // prod for identical migrations, so recording it made `schema_migration` rows
            // environment-specific (M13).
            migrations.push(Migration {
                version,
                description,
                script_path: filename,
                sql,
                transactional,
            });
        }

        migrations.sort_by(|a, b| compare_versions(&a.version, &b.version));

        check_no_duplicate_versions(&migrations)?;

        Ok(migrations)
    }

    fn repeatable_migrations(&self) -> Result<Vec<RepeatableMigration>, SchemaInstallerError> {
        let mut repeatables = Vec::new();
        let (files, _) = self.scan()?;

        for (filename, path) in files {
            let Some(name) = parse_repeatable_filename(&filename)? else {
                continue;
            };
            let sql = std::fs::read_to_string(&path).map_err(SchemaInstallerError::Io)?;
            let transactional = read_migration_config(&path)?;

            repeatables.push(RepeatableMigration {
                name,
                script_path: filename,
                sql,
                transactional,
            });
        }

        repeatables.sort_by(|a, b| a.name.cmp(&b.name));

        check_no_duplicate_repeatables(&repeatables)?;

        Ok(repeatables)
    }
}

/// Reads the optional Flyway-style `<script>.conf` sitting next to a migration script
/// (`V3__add_index.sql` -> `V3__add_index.sql.conf`) and returns whether the migration
/// should run in a transaction (M18). Format: `key=value` lines, `#`/`!` comments and blank
/// lines ignored. The only recognised key is `executeInTransaction` (`true`/`false`,
/// default `true`). Anything else - an unknown key, a malformed line, a non-boolean value -
/// is a hard error rather than being ignored, since a typo'd flag would otherwise silently
/// run a `CREATE INDEX CONCURRENTLY` migration inside a transaction.
fn read_migration_config(script: &std::path::Path) -> Result<bool, SchemaInstallerError> {
    let mut conf_name = script.as_os_str().to_owned();
    conf_name.push(".conf");
    let conf_path = PathBuf::from(conf_name);
    if !conf_path.is_file() {
        return Ok(true);
    }
    let text = std::fs::read_to_string(&conf_path).map_err(SchemaInstallerError::Io)?;
    parse_migration_config(&text).map_err(|e| {
        SchemaInstallerError::InvalidConfiguration(format!("{}: {}", conf_path.display(), e))
    })
}

fn parse_migration_config(text: &str) -> Result<bool, String> {
    let mut transactional = true;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected `key=value`, found `{}`", index + 1, line))?;
        match key.trim() {
            "executeInTransaction" => {
                transactional = match value.trim() {
                    "true" => true,
                    "false" => false,
                    other => {
                        return Err(format!(
                            "line {}: executeInTransaction must be `true` or `false`, found `{}`",
                            index + 1,
                            other
                        ));
                    }
                };
            }
            other => {
                return Err(format!(
                    "line {}: unknown setting `{}` (supported: executeInTransaction)",
                    index + 1,
                    other
                ));
            }
        }
    }
    Ok(transactional)
}

/// A source of migrations held in memory, e.g. compiled in with `include_str!`. Construct it
/// with `..Default::default()` so new fields don't break the literal.
#[derive(Default)]
pub struct EmbeddedMigrationSource {
    pub migrations: Vec<Migration>,
    pub repeatables: Vec<RepeatableMigration>,
}

impl MigrationSource for EmbeddedMigrationSource {
    fn migrations(&self) -> Result<Vec<Migration>, SchemaInstallerError> {
        check_no_duplicate_versions(&self.migrations)?;

        Ok(self.migrations.clone())
    }

    fn repeatable_migrations(&self) -> Result<Vec<RepeatableMigration>, SchemaInstallerError> {
        check_no_duplicate_repeatables(&self.repeatables)?;

        // Sorted here rather than trusting the caller's order, like the directory source:
        // repeatables run in name order.
        let mut repeatables = self.repeatables.clone();
        repeatables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(repeatables)
    }
}

/// Vendor subfolder names recognised by [`resolve_migrations_dir`] (L21).
const VENDOR_DIRS: [&str; 3] = ["postgresql", "sqlserver", "sqlite"];

fn vendor_dir_name(database_type: &GeneratorType) -> &'static str {
    match database_type {
        GeneratorType::Postgresql => "postgresql",
        GeneratorType::SqlServer => "sqlserver",
        GeneratorType::Sqlite => "sqlite",
    }
}

/// Resolves `--migrations-dir` to the directory actually scanned (L21). The layout is
/// detected from the directory's contents:
///
/// - `.sql` files directly inside, no vendor subfolders: flat layout, returned unchanged.
/// - vendor subfolders (`postgresql/`, `sqlserver/`, `sqlite/`) and no direct `.sql` files:
///   vendor layout, resolves to `{dir}/{database-type}/`; a missing folder for the connected
///   database is an error.
/// - both: ambiguous, an error.
/// - neither: returned unchanged, so the normal empty/missing-directory handling applies.
pub fn resolve_migrations_dir(
    path: &Path,
    database_type: &GeneratorType,
) -> Result<PathBuf, SchemaInstallerError> {
    if !path.is_dir() {
        return Ok(path.to_path_buf());
    }

    let mut has_sql_files = false;
    let mut vendor_dirs: Vec<&str> = Vec::new();
    for entry in std::fs::read_dir(path).map_err(SchemaInstallerError::Io)? {
        let entry_path = entry.map_err(SchemaInstallerError::Io)?.path();
        let name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if entry_path.is_dir() {
            if let Some(vendor) = VENDOR_DIRS.iter().find(|v| **v == name) {
                vendor_dirs.push(vendor);
            }
        } else if entry_path.is_file() && name.ends_with(".sql") {
            has_sql_files = true;
        }
    }

    match (has_sql_files, vendor_dirs.is_empty()) {
        (true, false) => Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Ambiguous migrations directory {:?}: it contains .sql files directly and also \
             vendor subfolders ({}). Use either one flat directory or per-database subfolders, \
             not both.",
            path,
            vendor_dirs.join(", ")
        ))),
        (false, false) => {
            let wanted = vendor_dir_name(database_type);
            if vendor_dirs.contains(&wanted) {
                Ok(path.join(wanted))
            } else {
                Err(SchemaInstallerError::InvalidConfiguration(format!(
                    "Migrations directory {:?} uses per-database subfolders ({}) but has no \
                     '{}' folder for the connected database",
                    path,
                    vendor_dirs.join(", "),
                    wanted
                )))
            }
        }
        _ => Ok(path.to_path_buf()),
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
        if let Some(first_path) =
            seen.insert(migration.version.as_str(), migration.script_path.as_str())
        {
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
/// e.g. an undo (`U__...`) migration, or any other file someone dropped into the directory
/// (repeatable `R__...` files are routed to [`parse_repeatable_filename`] by the directory
/// source before this is called) - so the caller can skip it with a warning
/// instead of aborting the entire directory scan over a file this tool was never going
/// to manage (H11). A file that *does* start with `V` but is otherwise malformed still
/// hard-errors, since that shape is far more likely to be a typo in a real migration
/// than an intentional non-versioned file.
pub(crate) fn parse_migration_filename(
    filename: &str,
) -> Result<Option<(String, String)>, SchemaInstallerError> {
    let name_without_ext = filename.strip_suffix(".sql").ok_or_else(|| {
        SchemaInstallerError::InvalidConfiguration(format!(
            "File does not end with .sql: {}",
            filename
        ))
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
    let description = parts
        .get(1)
        .map(|d| d.replace('_', " "))
        .unwrap_or_default();

    if version.is_empty() {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Migration version cannot be empty: {}",
            filename
        )));
    }

    // Reject anything that isn't a plain dot/underscore-separated run of numbers (e.g.
    // "1", "1.2", "1_2"). Without this, a typo like `Vfinal__x.sql` or `V1a__x.sql`
    // would parse "successfully" into a version whose non-numeric segments
    // `compare_versions` then silently drops, sorting it before every real version
    // instead of failing fast with a clear error.
    if !version
        .split(['.', '_'])
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Migration version must be a dot/underscore-separated list of numbers (e.g. 'V1', 'V1.2', 'V1_2'), got '{}': {}",
            version, filename
        )));
    }

    // A leading zero (`V01` vs `V1`) would compare Equal in `compare_versions` yet remain
    // a distinct string for the "already applied" lookup, so both files would be applied
    // in nondeterministic order. Reject rather than normalize (M19).
    if version
        .split(['.', '_'])
        .any(|part| part.len() > 1 && part.starts_with('0'))
    {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Migration version segments must not have leading zeros (got '{}'): {}",
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

/// Longest repeatable name that fits `schema_repeatable_migration.name` (`NVARCHAR(200)` on
/// SQL Server, where it is a unique-key column).
const MAX_REPEATABLE_NAME_LEN: usize = 200;

/// Whether `filename` is *meant* to be a repeatable migration: `R__...` (any case), or the
/// near-misses `R.sql` / `R_...` that [`parse_repeatable_filename`] rejects with a clear
/// error instead of silently skipping. Deliberately narrow so an unrelated file like
/// `readme.sql` is not claimed.
fn is_repeatable_filename(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower == "r.sql" || lower.starts_with("r_")
}

/// Parses an `R__{name}.sql` filename into its name. Returns `Ok(None)` when the file isn't
/// repeatable-shaped at all (see [`is_repeatable_filename`]); a repeatable-shaped file that is
/// malformed (`R.sql`, `R_foo.sql`, `R__.sql`) is a hard error rather than a silent skip,
/// since it is almost certainly a typo in a real migration.
pub(crate) fn parse_repeatable_filename(
    filename: &str,
) -> Result<Option<String>, SchemaInstallerError> {
    if !is_repeatable_filename(filename) {
        return Ok(None);
    }

    let stem = filename.strip_suffix(".sql").ok_or_else(|| {
        SchemaInstallerError::InvalidConfiguration(format!(
            "File does not end with .sql: {}",
            filename
        ))
    })?;

    let name = stem
        .get(..3)
        .filter(|prefix| *prefix == "R__" || *prefix == "r__");
    let name = match name {
        Some(_) => &stem[3..],
        None => {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "Repeatable migration filenames must look like R__{{description}}.sql: {}",
                filename
            )));
        }
    };

    if name.trim().is_empty() {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Repeatable migration description cannot be empty: {}",
            filename
        )));
    }

    if name.chars().count() > MAX_REPEATABLE_NAME_LEN {
        return Err(SchemaInstallerError::InvalidConfiguration(format!(
            "Repeatable migration description is longer than {} characters: {}",
            MAX_REPEATABLE_NAME_LEN, filename
        )));
    }

    Ok(Some(name.to_string()))
}

/// Hard-errors on repeatable migrations whose names collide, ignoring case (e.g.
/// `R__views.sql` and `r__Views.sql`), mirroring [`check_no_duplicate_versions`].
fn check_no_duplicate_repeatables(
    repeatables: &[RepeatableMigration],
) -> Result<(), SchemaInstallerError> {
    let mut seen: std::collections::HashMap<String, &str> = std::collections::HashMap::new();

    for repeatable in repeatables {
        if let Some(first_path) = seen.insert(
            repeatable.name.to_lowercase(),
            repeatable.script_path.as_str(),
        ) {
            return Err(SchemaInstallerError::InvalidConfiguration(format!(
                "Duplicate repeatable migration '{}': {} and {}",
                repeatable.name, first_path, repeatable.script_path
            )));
        }
    }

    Ok(())
}

/// Compares two version strings segment by segment as arbitrary-length unsigned integers.
/// Segments are compared as digit strings (after stripping leading zeros: shorter is
/// smaller, else lexicographic) rather than parsed into a fixed-width integer, so a long
/// timestamp version (e.g. 20 digits) can't overflow and silently drop out of the
/// comparison (M19).
pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    // Version strings may use either `.` (e.g. "1.2") or `_` (e.g. "1_2", produced by a
    // filename like `V1_2__add_email_column.sql`) as the separator, so both must be
    // split on here, or multipart underscore versions silently compare as equal.
    let split = |v: &str| -> Vec<String> {
        v.split(['.', '_'])
            .map(|p| p.trim_start_matches('0').to_string())
            .collect()
    };
    let v1_parts = split(v1);
    let v2_parts = split(v2);

    for (p1, p2) in v1_parts.iter().zip(v2_parts.iter()) {
        let ord = p1.len().cmp(&p2.len()).then_with(|| p1.cmp(p2));
        if ord != std::cmp::Ordering::Equal {
            return ord;
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

    #[test]
    fn config_defaults_to_transactional() {
        assert_eq!(parse_migration_config(""), Ok(true));
        assert_eq!(parse_migration_config("# just a comment\n\n"), Ok(true));
    }

    #[test]
    fn config_can_disable_transaction() {
        assert_eq!(
            parse_migration_config("# needs autocommit\nexecuteInTransaction = false\n"),
            Ok(false)
        );
        assert_eq!(
            parse_migration_config("executeInTransaction=true"),
            Ok(true)
        );
    }

    #[test]
    fn config_rejects_unknown_key_bad_value_and_malformed_line() {
        assert!(parse_migration_config("executeInTransacton=false").is_err());
        assert!(parse_migration_config("executeInTransaction=no").is_err());
        assert!(parse_migration_config("executeInTransaction").is_err());
    }
    use super::*;

    #[test]
    fn test_parse_migration_filename() {
        let (version, description) = parse_migration_filename("V1__create_users.sql")
            .unwrap()
            .unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "create users");

        let (version, description) = parse_migration_filename("V1_2__add_email_column.sql")
            .unwrap()
            .unwrap();
        assert_eq!(version, "1_2");
        assert_eq!(description, "add email column");
    }

    #[test]
    fn test_parse_migration_filename_without_description() {
        // The `__description` suffix is optional (e.g. produced by
        // schema-migration-generator's --auto-generate-name with no --description).
        let (version, description) = parse_migration_filename("V20240115143022.sql")
            .unwrap()
            .unwrap();
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
        let (version, description) = parse_migration_filename("v1__create_users.sql")
            .unwrap()
            .unwrap();
        assert_eq!(version, "1");
        assert_eq!(description, "create users");
    }

    #[test]
    fn test_parse_migration_filename_skips_non_versioned_files() {
        // Flyway-style repeatable (`R__...`) and undo (`U__...`) migrations, and any
        // other file that doesn't start with `V`, aren't versioned migrations this tool
        // manages - they must be skipped (Ok(None)) rather than aborting the whole
        // directory scan (H11).
        assert!(
            parse_migration_filename("R__refresh_view.sql")
                .unwrap()
                .is_none()
        );
        assert!(
            parse_migration_filename("U__undo_something.sql")
                .unwrap()
                .is_none()
        );
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
        assert!(
            err.to_string()
                .contains("must be a dot/underscore-separated list of numbers")
        );

        let err = parse_migration_filename("V1a__do_something.sql").unwrap_err();
        assert!(
            err.to_string()
                .contains("must be a dot/underscore-separated list of numbers")
        );
    }

    #[test]
    fn test_compare_versions_long_timestamp_does_not_overflow() {
        // 20 digits, larger than u64::MAX (18446744073709551615).
        let big = "99999999999999999999";
        assert_eq!(compare_versions(big, "1"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("1", big), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions(big, "20240115143022"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions(big, big), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_parse_migration_filename_rejects_leading_zeros() {
        assert!(parse_migration_filename("V01__x.sql").is_err());
        assert!(parse_migration_filename("V1.02__x.sql").is_err());
        assert!(parse_migration_filename("V1_007__x.sql").is_err());
        // A bare "0" segment is still fine (e.g. 1.0).
        assert!(parse_migration_filename("V1.0__x.sql").is_ok());
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
                transactional: true,
                version: "1".to_string(),
                description: "add users".to_string(),
                script_path: "V1__add_users.sql".to_string(),
                sql: String::new(),
            },
            Migration {
                transactional: true,
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
                transactional: true,
                version: "1".to_string(),
                description: "add users".to_string(),
                script_path: "V1__add_users.sql".to_string(),
                sql: String::new(),
            },
            Migration {
                transactional: true,
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
        std::fs::write(
            dir.path().join("V1__add_users.sql"),
            "CREATE TABLE users (id INT);",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("V1__add_orders.sql"),
            "CREATE TABLE orders (id INT);",
        )
        .unwrap();

        let source = DirectoryMigrationSource {
            path: dir.path().to_path_buf(),
        };
        let err = source.migrations().unwrap_err();
        assert!(err.to_string().contains("Duplicate migration version '1'"));
    }

    #[test]
    fn test_resolve_migrations_dir_layouts() {
        let flat = tempfile::tempdir().unwrap();
        std::fs::write(flat.path().join("V1__a.sql"), "x").unwrap();
        assert_eq!(
            resolve_migrations_dir(flat.path(), &GeneratorType::Postgresql).unwrap(),
            flat.path()
        );

        // Non-vendor subfolders don't trigger vendor mode.
        std::fs::create_dir(flat.path().join("archive")).unwrap();
        assert_eq!(
            resolve_migrations_dir(flat.path(), &GeneratorType::Sqlite).unwrap(),
            flat.path()
        );

        let vendor = tempfile::tempdir().unwrap();
        std::fs::create_dir(vendor.path().join("postgresql")).unwrap();
        std::fs::create_dir(vendor.path().join("sqlserver")).unwrap();
        assert_eq!(
            resolve_migrations_dir(vendor.path(), &GeneratorType::SqlServer).unwrap(),
            vendor.path().join("sqlserver")
        );
        let err = resolve_migrations_dir(vendor.path(), &GeneratorType::Sqlite).unwrap_err();
        assert!(err.to_string().contains("no 'sqlite' folder"));

        std::fs::write(vendor.path().join("V1__a.sql"), "x").unwrap();
        let err = resolve_migrations_dir(vendor.path(), &GeneratorType::Postgresql).unwrap_err();
        assert!(err.to_string().contains("Ambiguous"));

        let empty = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_migrations_dir(empty.path(), &GeneratorType::Postgresql).unwrap(),
            empty.path()
        );
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

    #[test]
    fn test_parse_repeatable_filename() {
        assert_eq!(
            parse_repeatable_filename("R__refresh_views.sql").unwrap(),
            Some("refresh_views".to_string())
        );
        // Lowercase prefix accepted, like `v`; the name is kept as written.
        assert_eq!(
            parse_repeatable_filename("r__Grants.sql").unwrap(),
            Some("Grants".to_string())
        );
    }

    #[test]
    fn test_parse_repeatable_filename_ignores_unrelated_files() {
        assert_eq!(parse_repeatable_filename("readme.sql").unwrap(), None);
        assert_eq!(parse_repeatable_filename("V1__x.sql").unwrap(), None);
        assert_eq!(parse_repeatable_filename("U__undo.sql").unwrap(), None);
    }

    #[test]
    fn test_parse_repeatable_filename_rejects_overlong_names() {
        let ok = format!("R__{}.sql", "a".repeat(MAX_REPEATABLE_NAME_LEN));
        assert!(parse_repeatable_filename(&ok).unwrap().is_some());
        let too_long = format!("R__{}.sql", "a".repeat(MAX_REPEATABLE_NAME_LEN + 1));
        assert!(parse_repeatable_filename(&too_long).is_err());
    }

    #[test]
    fn test_parse_repeatable_filename_rejects_malformed_names() {
        for bad in ["R.sql", "R_foo.sql", "R__.sql", "R__  .sql"] {
            assert!(
                parse_repeatable_filename(bad).is_err(),
                "{bad} should error"
            );
        }
    }

    #[test]
    fn test_repeatable_description_replaces_underscores() {
        let migration = RepeatableMigration {
            name: "refresh_views".to_string(),
            script_path: "R__refresh_views.sql".to_string(),
            sql: String::new(),
            transactional: true,
        };
        assert_eq!(migration.description(), "refresh views");
    }

    #[test]
    fn test_check_no_duplicate_repeatables_ignores_case() {
        let make = |name: &str, path: &str| RepeatableMigration {
            name: name.to_string(),
            script_path: path.to_string(),
            sql: String::new(),
            transactional: true,
        };
        assert!(
            check_no_duplicate_repeatables(&[make("a", "R__a.sql"), make("b", "R__b.sql")]).is_ok()
        );
        let err = check_no_duplicate_repeatables(&[
            make("Views", "R__Views.sql"),
            make("views", "r__views.sql"),
        ])
        .unwrap_err();
        assert!(err.to_string().contains("Duplicate repeatable migration"));
    }

    fn write(dir: &Path, name: &str, contents: &str) {
        std::fs::write(dir.join(name), contents).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("schema-installer-{}-{}", label, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_directory_source_splits_versioned_and_repeatable() {
        let dir = temp_dir("split");
        write(&dir, "V1__create.sql", "create table t (id int);");
        write(&dir, "R__b_views.sql", "create view b as select 1;");
        write(&dir, "R__a_grants.sql", "select 1;");
        write(&dir, "readme.sql", "-- ignored");
        let source = DirectoryMigrationSource { path: dir.clone() };

        let versioned = source.migrations().unwrap();
        assert_eq!(versioned.len(), 1);
        assert_eq!(versioned[0].version, "1");

        let repeatables = source.repeatable_migrations().unwrap();
        let names: Vec<&str> = repeatables.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["a_grants", "b_views"]);
        assert_eq!(repeatables[0].script_path, "R__a_grants.sql");
        assert!(repeatables[0].transactional);

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_directory_source_repeatable_honours_conf_sidecar() {
        let dir = temp_dir("conf");
        write(&dir, "R__vacuum.sql", "vacuum;");
        write(&dir, "R__vacuum.sql.conf", "executeInTransaction=false\n");
        let source = DirectoryMigrationSource { path: dir.clone() };

        let repeatables = source.repeatable_migrations().unwrap();
        assert!(!repeatables[0].transactional);

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_directory_source_rejects_malformed_and_duplicate_repeatables() {
        let dir = temp_dir("bad");
        write(&dir, "R_oops.sql", "select 1;");
        let source = DirectoryMigrationSource { path: dir.clone() };
        assert!(source.repeatable_migrations().is_err());
        // The versioned scan must leave repeatable-shaped files to the other method.
        assert!(source.migrations().unwrap().is_empty());
        std::fs::remove_file(dir.join("R_oops.sql")).unwrap();

        write(&dir, "R__views.sql", "select 1;");
        write(&dir, "r__Views.sql", "select 2;");
        let result = source.repeatable_migrations();
        // On a case-insensitive filesystem the second write overwrites the first, so only
        // assert the error where both files can coexist.
        if std::fs::read_dir(&dir).unwrap().count() == 2 {
            assert!(result.is_err());
        }

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_embedded_source_returns_repeatables_sorted_and_rejects_duplicates() {
        let make = |name: &str| RepeatableMigration {
            name: name.to_string(),
            script_path: format!("R__{name}.sql"),
            sql: String::new(),
            transactional: true,
        };
        let source = EmbeddedMigrationSource {
            repeatables: vec![make("b"), make("a")],
            ..Default::default()
        };
        let names: Vec<String> = source
            .repeatable_migrations()
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, ["a", "b"]);

        let duplicate = EmbeddedMigrationSource {
            repeatables: vec![make("Views"), make("views")],
            ..Default::default()
        };
        assert!(duplicate.repeatable_migrations().is_err());
    }

    #[test]
    fn test_default_source_has_no_repeatables() {
        let source = EmbeddedMigrationSource {
            migrations: vec![],
            ..Default::default()
        };
        assert!(source.repeatable_migrations().unwrap().is_empty());
    }
}
