use std::io::Write;

use schema_diff::ChangeSet;

use crate::error::MigrationGeneratorError;

pub trait MigrationGenerator {
    fn generate(&self, change_set: &ChangeSet, writer: &mut dyn Write) -> Result<(), MigrationGeneratorError>;
}
