pub mod auto_name;
pub mod check_constraint;
pub mod error;
pub mod migration_generator;
pub mod generator_factory;
pub mod postgresql;
pub mod sqlserver;
pub mod sqlite;
pub mod source;

pub use auto_name::{generate_migration_filename, generate_migration_path};
pub use error::MigrationGeneratorError;
pub use generator_factory::create_generator;
pub use migration_generator::MigrationGenerator;

#[cfg(test)]
mod tests;
