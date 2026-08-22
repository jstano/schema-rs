use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaInstallerError {
    #[error("Database connection error: {0}")]
    Connection(String),

    #[error("Schema parse error: {0}")]
    Parse(String),

    #[error("SQL generation error: {0}")]
    Generation(String),

    #[error("SQL execution error: {0}")]
    Execution(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Schema file not found: {0}")]
    SchemaFileNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Checksum mismatch for migration {version}: expected {expected}, found {found}")]
    ChecksumMismatch {
        version: String,
        expected: String,
        found: String,
    },

    #[error("Migration failed for version {version}: {error}")]
    MigrationFailed { version: String, error: String },

    #[error(
        "Concurrent migration detected for version {0}: another process has already applied or is currently applying it"
    )]
    ConcurrentMigrationDetected(String),

    #[error(
        "Timed out waiting for a concurrent process to finish applying migration {0}; it may have crashed while holding it"
    )]
    LockTimeout(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error(
        "Migration {version} (script {script_path}) was applied but its source file no longer exists in the migrations directory"
    )]
    MissingMigrationSource { version: String, script_path: String },

    #[error(
        "Out-of-order migration detected: {version} - {description} has not been applied, but version {highest_applied} has already been applied; refusing to apply it out of order"
    )]
    OutOfOrderMigration {
        version: String,
        description: String,
        highest_applied: String,
    },
}
