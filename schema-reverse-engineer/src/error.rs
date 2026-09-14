use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaReverseEngineerError {
    #[error("Database connection error: {0}")]
    Connection(String),

    #[error("Database introspection error: {0}")]
    Introspection(String),

    #[error("Unsupported column type: {0}")]
    UnsupportedColumnType(String),

    #[error(
        "table '{0}' has unique keys and/or indexes but no primary key -- the schema-rs XML \
         format requires a <primary> element whenever <keys> is present, so these cannot be \
         represented. Add a primary key to the table, or drop its unique keys/indexes, before \
         reverse-engineering it."
    )]
    PkLessTableHasKeys(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
