use crate::common::generate_options::GenerateOptions;
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::SqlGenerator;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;
use crate::postgresql::postgres_generator::PostgresGenerator;
use crate::sqlite::sqlite_generator::SqliteGenerator;
use crate::sqlserver::sqlserver_generator::SqlServerGenerator;
use schema_model::model::types::DatabaseType;
use std::str::FromStr;

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
            "postgres" | "postgresql" => Ok(GeneratorType::Postgresql),
            "sqlite" => Ok(GeneratorType::Sqlite),
            "sqlserver" => Ok(GeneratorType::SqlServer),
            _ => Err(format!("Unknown generator type: {}", s)),
        }
    }
}
