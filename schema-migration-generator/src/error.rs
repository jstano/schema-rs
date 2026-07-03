use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationGeneratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unsupported change for database: {0}")]
    UnsupportedChange(String),
}
