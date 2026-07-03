use clap::{Parser, Subcommand};
use schema_installer::{DirectoryMigrationSource, Migrator, SchemaInstaller, SchemaInstallerConfigBuilder};
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use schema_sql_generator::common::generator_type::GeneratorType;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "schema-installer")]
#[command(about = "Manage database schemas using migrations or XML definitions")]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, help = "Database type (postgres, sqlite, sqlserver)")]
    database_type: Option<String>,

    #[arg(long, global = true, help = "Database connection string")]
    connection_string: Option<String>,

    #[arg(long, global = true, default_value = "native", help = "Boolean mode (native, yesno, yn)")]
    boolean_mode: String,

    #[arg(long, global = true, default_value = "relations", help = "Foreign key mode (none, relations, triggers)")]
    foreign_key_mode: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Apply pending migrations
    Migrate {
        #[arg(long, help = "Path to migrations directory")]
        migrations_dir: PathBuf,
    },
    /// Show migration status
    Info {
        #[arg(long, help = "Path to migrations directory")]
        migrations_dir: PathBuf,
    },
    /// Verify migration checksums
    Validate {
        #[arg(long, help = "Path to migrations directory")]
        migrations_dir: PathBuf,
    },
    /// Fix failed or mismatched migrations
    Repair {
        #[arg(long, help = "Path to migrations directory")]
        migrations_dir: PathBuf,
    },
    /// Install schema from XML (legacy)
    Install {
        #[arg(long, help = "Path to XML schema file")]
        schema_file: PathBuf,
    },
    /// Check if there are pending migrations (exits 0 = none, 1 = pending)
    PendingCheck {
        #[arg(long, help = "Path to migrations directory")]
        migrations_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let database_type_str = args.database_type.as_ref()
        .ok_or("Error: --database-type is required")?;
    let connection_string = args.connection_string.as_ref()
        .ok_or("Error: --connection-string is required")?;

    let database_type = parse_database_type(database_type_str)?;
    let boolean_mode = parse_boolean_mode(&args.boolean_mode)?;
    let foreign_key_mode = parse_foreign_key_mode(&args.foreign_key_mode)?;

    match args.command {
        Commands::Migrate { migrations_dir } => {
            let config = SchemaInstallerConfigBuilder::new()
                .database_type(database_type)
                .connection_string(connection_string.clone())
                .boolean_mode(boolean_mode)
                .foreign_key_mode(foreign_key_mode)
                .build()?;

            let source = Box::new(DirectoryMigrationSource { path: migrations_dir });
            Migrator::migrate(&config, source).await?;
        }
        Commands::Info { migrations_dir } => {
            let config = SchemaInstallerConfigBuilder::new()
                .database_type(database_type)
                .connection_string(connection_string.clone())
                .boolean_mode(boolean_mode)
                .foreign_key_mode(foreign_key_mode)
                .build()?;

            let source = Box::new(DirectoryMigrationSource { path: migrations_dir });
            Migrator::info(&config, source).await?;
        }
        Commands::Validate { migrations_dir } => {
            let config = SchemaInstallerConfigBuilder::new()
                .database_type(database_type)
                .connection_string(connection_string.clone())
                .boolean_mode(boolean_mode)
                .foreign_key_mode(foreign_key_mode)
                .build()?;

            let source = Box::new(DirectoryMigrationSource { path: migrations_dir });
            Migrator::validate(&config, source).await?;
        }
        Commands::Repair { migrations_dir } => {
            let config = SchemaInstallerConfigBuilder::new()
                .database_type(database_type)
                .connection_string(connection_string.clone())
                .boolean_mode(boolean_mode)
                .foreign_key_mode(foreign_key_mode)
                .build()?;

            let source = Box::new(DirectoryMigrationSource { path: migrations_dir });
            Migrator::repair(&config, source).await?;
        }
        Commands::PendingCheck { migrations_dir } => {
            let config = SchemaInstallerConfigBuilder::new()
                .database_type(database_type)
                .connection_string(connection_string.clone())
                .boolean_mode(boolean_mode)
                .foreign_key_mode(foreign_key_mode)
                .build()?;

            let source = Box::new(DirectoryMigrationSource { path: migrations_dir });
            let pending = Migrator::has_pending_migrations(&config, source).await?;
            if pending {
                println!("Pending migrations exist");
                std::process::exit(1);
            } else {
                println!("No pending migrations");
            }
        }
        Commands::Install { schema_file } => {
            let config = SchemaInstallerConfigBuilder::new()
                .database_type(database_type)
                .connection_string(connection_string.clone())
                .schema_file(schema_file)
                .boolean_mode(boolean_mode)
                .foreign_key_mode(foreign_key_mode)
                .build()?;

            SchemaInstaller::install(&config).await?;
        }
    }

    Ok(())
}

fn parse_database_type(db_type: &str) -> Result<GeneratorType, Box<dyn std::error::Error>> {
    match db_type.to_lowercase().as_str() {
        "postgres" | "postgresql" => Ok(GeneratorType::Postgresql),
        "sqlite" => Ok(GeneratorType::Sqlite),
        "sqlserver" | "mssql" => Ok(GeneratorType::SqlServer),
        _ => {
            eprintln!("Error: Unknown database type '{}'. Supported: postgres, sqlite, sqlserver", db_type);
            std::process::exit(1);
        }
    }
}

fn parse_boolean_mode(mode: &str) -> Result<BooleanMode, Box<dyn std::error::Error>> {
    match mode.to_lowercase().as_str() {
        "native" => Ok(BooleanMode::Native),
        "yesno" | "yes_no" => Ok(BooleanMode::YesNo),
        "yn" => Ok(BooleanMode::YN),
        _ => {
            eprintln!("Error: Unknown boolean mode '{}'. Supported: native, yesno, yn", mode);
            std::process::exit(1);
        }
    }
}

fn parse_foreign_key_mode(mode: &str) -> Result<ForeignKeyMode, Box<dyn std::error::Error>> {
    match mode.to_lowercase().as_str() {
        "none" => Ok(ForeignKeyMode::None),
        "relations" => Ok(ForeignKeyMode::Relations),
        "triggers" => Ok(ForeignKeyMode::Triggers),
        _ => {
            eprintln!("Error: Unknown foreign key mode '{}'. Supported: none, relations, triggers", mode);
            std::process::exit(1);
        }
    }
}
