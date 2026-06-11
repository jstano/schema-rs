use clap::Parser;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_installer::{SchemaInstallerConfigBuilder, SchemaInstaller};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "schema-installer")]
#[command(about = "Install database schemas from XML definitions", long_about = None)]
struct Args {
    #[arg(long, help = "Database type (postgres, sqlite, sqlserver)")]
    database_type: String,

    #[arg(long, help = "Database connection string")]
    connection_string: String,

    #[arg(long, help = "Path to XML schema file")]
    schema_file: PathBuf,

    #[arg(long, default_value = "native", help = "Boolean mode (native, yesno, yn)")]
    boolean_mode: String,

    #[arg(long, default_value = "relations", help = "Foreign key mode (none, relations, triggers)")]
    foreign_key_mode: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Parse database type
    let database_type = match args.database_type.to_lowercase().as_str() {
        "postgres" | "postgresql" => GeneratorType::Postgres,
        "sqlite" => GeneratorType::Sqlite,
        "sqlserver" | "mssql" => GeneratorType::SqlServer,
        _ => {
            eprintln!("Error: Unknown database type '{}'. Supported: postgres, sqlite, sqlserver", args.database_type);
            std::process::exit(1);
        }
    };

    // Parse boolean mode
    let boolean_mode = match args.boolean_mode.to_lowercase().as_str() {
        "native" => BooleanMode::Native,
        "yesno" | "yes_no" => BooleanMode::YesNo,
        "yn" => BooleanMode::YN,
        _ => {
            eprintln!("Error: Unknown boolean mode '{}'. Supported: native, yesno, yn", args.boolean_mode);
            std::process::exit(1);
        }
    };

    // Parse foreign key mode
    let foreign_key_mode = match args.foreign_key_mode.to_lowercase().as_str() {
        "none" => ForeignKeyMode::None,
        "relations" => ForeignKeyMode::Relations,
        "triggers" => ForeignKeyMode::Triggers,
        _ => {
            eprintln!("Error: Unknown foreign key mode '{}'. Supported: none, relations, triggers", args.foreign_key_mode);
            std::process::exit(1);
        }
    };

    // Build config
    let config = SchemaInstallerConfigBuilder::new()
        .database_type(database_type)
        .connection_string(args.connection_string)
        .schema_file(args.schema_file)
        .boolean_mode(boolean_mode)
        .foreign_key_mode(foreign_key_mode)
        .build()?;

    // Install schema
    SchemaInstaller::install(&config).await?;

    Ok(())
}
