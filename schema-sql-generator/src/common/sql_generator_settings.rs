use crate::common::generate_options::GenerateOptions;
use crate::common::output_mode::OutputMode;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct SqlGeneratorSettings {
    database_type: DatabaseType,
    database_model: Rc<DatabaseModel>,
    statement_separator: String,
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    output_mode: OutputMode,
    target_postgres_version: u32,
    emit_postgres_extensions: bool,
    extension_check_user: Option<String>,
}

impl SqlGeneratorSettings {
    pub fn new(database_type: DatabaseType, options: &GenerateOptions) -> Self {
        Self {
            database_type,
            database_model : options.database_model.clone(),
            statement_separator: database_type.statement_separator().to_string(),
            foreign_key_mode: options.foreign_key_mode,
            boolean_mode: options.boolean_mode,
            output_mode: options.output_mode,
            target_postgres_version: options.target_postgres_version,
            emit_postgres_extensions: options.emit_postgres_extensions,
            extension_check_user: options.extension_check_user.clone(),
        }
    }

    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }

    pub fn database_model(&self) -> &Rc<DatabaseModel> {
        &self.database_model
    }

    pub fn statement_separator(&self) -> &String {
        &self.statement_separator
    }

    pub fn foreign_key_mode(&self) -> ForeignKeyMode {
        self.foreign_key_mode
    }

    pub fn boolean_mode(&self) -> BooleanMode {
        self.boolean_mode
    }

    pub fn output_mode(&self) -> OutputMode {
        self.output_mode
    }

    pub fn target_postgres_version(&self) -> u32 {
        self.target_postgres_version
    }

    pub fn emit_postgres_extensions(&self) -> bool {
        self.emit_postgres_extensions
    }

    pub fn extension_check_user(&self) -> Option<&String> {
        self.extension_check_user.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::print_writer::PrintWriter;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, ForeignKeyMode};
    use std::cell::RefCell;

    #[test]
    fn statement_separator_matches_database_type_per_dialect() {
        // Postgres and SQLite batch statements with ";"; SQL Server uses "GO" batches.
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let options = GenerateOptions::new(
            std::rc::Rc::new(model),
            std::rc::Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );

        let postgres_settings = SqlGeneratorSettings::new(DatabaseType::Postgresql, &options);
        let sqlite_settings = SqlGeneratorSettings::new(DatabaseType::Sqlite, &options);
        let sqlserver_settings = SqlGeneratorSettings::new(DatabaseType::SqlServer, &options);

        assert_eq!(postgres_settings.statement_separator(), ";");
        assert_eq!(sqlite_settings.statement_separator(), ";");
        assert_eq!(sqlserver_settings.statement_separator(), "\nGO");
        assert_eq!(sqlserver_settings.statement_separator(), DatabaseType::SqlServer.statement_separator());
    }
}
