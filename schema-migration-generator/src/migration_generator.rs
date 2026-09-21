use std::io::Write;

use schema_diff::ChangeSet;
use schema_model::model::database_model::DatabaseModel;

use crate::error::MigrationGeneratorError;

pub trait MigrationGenerator {
    fn generate(
        &self,
        change_set: &ChangeSet,
        database_model: &DatabaseModel,
        writer: &mut dyn Write,
    ) -> Result<(), MigrationGeneratorError>;
}
