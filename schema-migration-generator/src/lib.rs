pub mod error;
pub mod migration_generator;
pub mod generator_factory;
pub mod postgresql;
pub mod sqlserver;
pub mod sqlite;

pub use error::MigrationGeneratorError;
pub use generator_factory::create_generator;
pub use migration_generator::MigrationGenerator;

#[cfg(test)]
mod tests;
