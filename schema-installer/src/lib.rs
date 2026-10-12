pub mod config;
pub mod connection;
pub mod error;
pub mod installer;
pub mod migration;
pub mod migrator;
pub(crate) mod sql_split;
pub mod tracking;

pub use config::{SchemaInstallerConfig, SchemaInstallerConfigBuilder};
pub use error::SchemaInstallerError;
pub use installer::{Baseline, SchemaInstaller};
pub use migration::{
    AppliedMigration, DirectoryMigrationSource, EmbeddedMigrationSource, Migration,
    MigrationSource, MigrationStatus, compute_checksum, resolve_migrations_dir,
};
pub use migrator::Migrator;
