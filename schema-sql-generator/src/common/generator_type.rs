use crate::common::generate_options::GenerateOptions;
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::SqlGenerator;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;
use crate::postgresql::postgres_generator::PostgresGenerator;
use crate::sqlite::sqlite_generator::SqliteGenerator;
use crate::sqlserver::sqlserver_generator::SqlServerGenerator;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::DatabaseType;
use std::str::FromStr;

/// Array element types `postgres_column_type_generator.rs::array_sql` knows how to render.
/// Kept in sync with that `match` by hand - see `validate_for_dialect`.
const POSTGRES_ARRAY_ELEMENT_TYPES: &[ColumnType] = &[
    ColumnType::Byte,
    ColumnType::Short,
    ColumnType::Int,
    ColumnType::Long,
    ColumnType::Decimal,
    ColumnType::Char,
    ColumnType::Varchar,
    ColumnType::Text,
];

pub enum GeneratorType {
    Postgresql,
    Sqlite,
    SqlServer,
}

impl GeneratorType {
    pub fn new_generator(&self, options: GenerateOptions) -> Box<dyn SqlGenerator> {
        let context = self.build_context(&options);
        match self {
            GeneratorType::Postgresql => Box::new(PostgresGenerator::new(context)),
            GeneratorType::Sqlite => Box::new(SqliteGenerator::new(context)),
            GeneratorType::SqlServer => Box::new(SqlServerGenerator::new(context)),
        }
    }

    pub fn generate(&self, options: GenerateOptions) {
        self.new_generator(options).generate();
    }

    fn name(&self) -> &'static str {
        match self {
            GeneratorType::Postgresql => "PostgreSQL",
            GeneratorType::Sqlite => "SQLite",
            GeneratorType::SqlServer => "SQL Server",
        }
    }

    /// Dialect-specific checks `DatabaseModel::validate()` can't make on its own, since it
    /// has no notion of which dialect will render the model - one schema.xml is meant to
    /// target all three databases (H23). Callers must run this in addition to
    /// `DatabaseModel::validate()` before calling `generate()`: several generator code
    /// paths (`enum_sql`, `array_sql`, `SqliteProcedureGenerator::output_procedure(s)`)
    /// panic on a violation here, on the assumption this already ran.
    pub fn validate_for_dialect(&self, database_model: &DatabaseModel) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();

        for table in database_model.all_tables() {
            for column in table.columns() {
                if column.column_type() != ColumnType::Array {
                    continue;
                }

                match self {
                    GeneratorType::Sqlite | GeneratorType::SqlServer => {
                        errors.push(format!(
                            "ERROR: {}.{} is an array column, but {} does not support array types",
                            table.name(),
                            column.name(),
                            self.name()
                        ));
                    }
                    GeneratorType::Postgresql => {
                        // A missing/unparseable elementType is already caught by
                        // `DatabaseModel::validate()`; here we only need to check that the
                        // (valid) element type is one PostgreSQL array generation supports.
                        if let Some(element_type_name) = column.element_type()
                            && let Ok(element_type) = ColumnType::from_type_name(element_type_name)
                            && !POSTGRES_ARRAY_ELEMENT_TYPES.contains(&element_type)
                        {
                            errors.push(format!(
                                "ERROR: {}.{} is an array column with elementType '{}', which PostgreSQL array \
                                 generation does not support (supported: byte, short, int, long, decimal, char, \
                                 varchar, text)",
                                table.name(),
                                column.name(),
                                element_type_name
                            ));
                        }
                    }
                }
            }
        }

        if matches!(self, GeneratorType::Sqlite) {
            let db_type = DatabaseType::Sqlite;
            for schema in database_model.schemas() {
                for procedure in schema.procedures() {
                    if procedure.database_type() == db_type {
                        errors.push(format!(
                            "ERROR: procedure '{}' targets SQLite, but SQLite does not support stored procedures",
                            procedure.name()
                        ));
                    }
                }
            }
        }

        // Note on M1 (`<index include="...">`/`<index compress="true">`,
        // `<primary|unique cluster="true">`): PostgreSQL and SQLite have no INCLUDE-on-
        // index, index compression, or clustered/nonclustered constraint syntax. Rather
        // than a hard error here, the generators themselves (`PostgresIndexGenerator`,
        // `SqliteIndexGenerator`, `DefaultKeyGenerator`) print a warning and ignore the
        // attribute on dialects that can't express it - this schema.xml is meant to target
        // all three databases from one file, and one dialect's SQL-Server-only tuning hint
        // shouldn't stop the other two from generating.

        errors
    }

    fn build_context(&self, options: &GenerateOptions) -> GeneratorContext {
        let db_type = match self {
            GeneratorType::Postgresql => DatabaseType::Postgresql,
            GeneratorType::Sqlite => DatabaseType::Sqlite,
            GeneratorType::SqlServer => DatabaseType::SqlServer,
        };
        GeneratorContext::new(
            SqlGeneratorSettings::new(db_type, options),
            SqlWriter::new(options.writer.clone()),
        )
    }
}

impl FromStr for GeneratorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" => Ok(GeneratorType::Postgresql),
            "sqlite" => Ok(GeneratorType::Sqlite),
            "sqlserver" => Ok(GeneratorType::SqlServer),
            _ => Err(format!("Unknown generator type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::procedure::Procedure;
    use schema_model::model::types::{BooleanMode, ForeignKeyMode};

    fn model_with_array_column(element_type: &str) -> DatabaseModel {
        let table = TableBuilder::new(Some("s"), "widget")
            .add_column(
                ColumnBuilder::new(Some("s"), "tags", ColumnType::Array)
                    .element_type(Some(element_type.to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(Some("s")).add_table(table).build();
        DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
    }

    #[test]
    fn validate_for_dialect_accepts_a_supported_array_element_type_on_postgres() {
        let model = model_with_array_column("varchar");
        assert!(GeneratorType::Postgresql.validate_for_dialect(&model).is_empty());
    }

    #[test]
    fn validate_for_dialect_rejects_an_unsupported_array_element_type_on_postgres() {
        // `postgres_column_type_generator.rs::array_sql` panics with
        // `Unsupported array element type` for anything outside its match arms (e.g.
        // uuid/boolean/date/json) - H23.
        let model = model_with_array_column("uuid");
        let errors = GeneratorType::Postgresql.validate_for_dialect(&model);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("tags"));
        assert!(errors[0].contains("uuid"));
    }

    #[test]
    fn validate_for_dialect_rejects_any_array_column_on_sqlite_and_sqlserver() {
        // `sqlite_column_type_generator.rs`/`sqlserver_column_type_generator.rs::array_sql`
        // panic unconditionally - arrays aren't supported at all on those dialects, even
        // for element types PostgreSQL supports (H23).
        let model = model_with_array_column("varchar");

        let sqlite_errors = GeneratorType::Sqlite.validate_for_dialect(&model);
        assert_eq!(sqlite_errors.len(), 1);
        assert!(sqlite_errors[0].contains("tags"));

        let sqlserver_errors = GeneratorType::SqlServer.validate_for_dialect(&model);
        assert_eq!(sqlserver_errors.len(), 1);
        assert!(sqlserver_errors[0].contains("tags"));
    }

    #[test]
    fn validate_for_dialect_rejects_a_sqlite_targeted_procedure_on_sqlite() {
        // `SqliteProcedureGenerator::output_procedures`/`output_procedure` panic
        // unconditionally when a `databaseType="sqlite"` procedure exists (H23).
        let procedure = Procedure::new(None::<&str>, "proc", DatabaseType::Sqlite, "select 1");
        let schema = SchemaBuilder::new(None::<&str>).add_procedures(vec![procedure]).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        let errors = GeneratorType::Sqlite.validate_for_dialect(&model);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("proc"));
    }

    #[test]
    fn validate_for_dialect_accepts_a_non_sqlite_procedure_on_sqlite() {
        let procedure = Procedure::new(None::<&str>, "proc", DatabaseType::Postgresql, "select 1");
        let schema = SchemaBuilder::new(None::<&str>).add_procedures(vec![procedure]).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        assert!(GeneratorType::Sqlite.validate_for_dialect(&model).is_empty());
    }
}
