use crate::common::generate_options::GenerateOptions;
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator_settings::SqlGeneratorSettings;
use crate::common::sql_writer::SqlWriter;
use crate::h2::h2_generator::H2Generator;
use crate::mysql::mysql_generator::MySqlGenerator;
use crate::postgresql::postgres_generator::PostgresGenerator;
use crate::sqlite::sqlite_generator::SqliteGenerator;
use crate::sqlserver::sqlserver_generator::SqlServerGenerator;
use schema_model::model::types::DatabaseType;
use std::str::FromStr;
use crate::common::sql_generator::SqlGenerator;

pub enum GeneratorType {
    H2,
    MySql,
    Postgres,
    Sqlite,
    SqlServer,
}

impl GeneratorType {
    pub fn new_generator(&self, options: GenerateOptions) -> Box<dyn SqlGenerator> {
        let context = self.build_context(&options);
        match self {
            GeneratorType::H2 => Box::new(H2Generator::new(context)),
            GeneratorType::MySql => Box::new(MySqlGenerator::new(context)),
            GeneratorType::Postgres => Box::new(PostgresGenerator::new(context)),
            GeneratorType::Sqlite => Box::new(SqliteGenerator::new(context)),
            GeneratorType::SqlServer => Box::new(SqlServerGenerator::new(context)),
        }
    }

    pub fn generate(&self, options: GenerateOptions) {
        self.new_generator(options).generate();
    }

    fn build_context(&self, options: &GenerateOptions) -> GeneratorContext {
        let db_type = match self {
            GeneratorType::H2 => DatabaseType::H2,
            GeneratorType::MySql => DatabaseType::Mysql,
            GeneratorType::Postgres => DatabaseType::Postgres,
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
            "h2" => Ok(GeneratorType::H2),
            "mysql" => Ok(GeneratorType::MySql),
            "postgres" => Ok(GeneratorType::Postgres),
            "sqlite" => Ok(GeneratorType::Sqlite),
            "sqlserver" => Ok(GeneratorType::SqlServer),
            _ => Err(format!("Unknown generator type: {}", s)),
        }
    }
}
