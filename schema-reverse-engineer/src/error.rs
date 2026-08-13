use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaReverseEngineerError {
    #[error("Database connection error: {0}")]
    Connection(String),

    #[error("Database introspection error: {0}")]
    Introspection(String),

    #[error("Unsupported column type: {0}")]
    UnsupportedColumnType(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
