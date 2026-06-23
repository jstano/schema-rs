pub mod config;
pub mod connection;
pub mod error;
pub mod installer;
pub mod tracking;
pub mod migration;
pub mod migrator;

pub use config::{SchemaInstallerConfig, SchemaInstallerConfigBuilder};
pub use error::SchemaInstallerError;
pub use installer::SchemaInstaller;
pub use migration::{Migration, MigrationSource, DirectoryMigrationSource, EmbeddedMigrationSource, AppliedMigration, MigrationStatus, compute_checksum};
pub use migrator::Migrator;
