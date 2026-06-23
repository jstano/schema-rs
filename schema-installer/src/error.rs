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
}
