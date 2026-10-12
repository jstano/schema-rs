use clap::{Parser, ValueEnum};
use schema_reverse_engineer::{read_schema, read_sqlserver_schema, sqlserver, write_database_xml};
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DatabaseKind {
    Postgresql,
    Sqlserver,
}

#[derive(Parser, Debug)]
#[command(name = "schema-reverse-engineer")]
#[command(
    about = "Reverse-engineer a live PostgreSQL or SQL Server database into a schema-rs XML schema definition"
)]
#[command(arg_required_else_help = true)]
struct Args {
    #[arg(long, value_enum, default_value = "postgresql")]
    database_type: DatabaseKind,

    #[arg(
        long,
        help = "Connection string: postgres://user:pass@host:5432/dbname, or for SQL Server an ADO string such as \"Server=tcp:host,1433;Database=db;User Id=sa;Password=...\""
    )]
    connection_string: String,

    #[arg(
        long,
        help = "Schema to introspect (default: public for PostgreSQL, dbo for SQL Server)"
    )]
    db_schema: Option<String>,

    #[arg(
        long,
        help = "SQL Server only: trust the server certificate without verifying it"
    )]
    trust_server_certificate: bool,

    #[arg(long, help = "Path to write the generated XML schema file to")]
    file: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let model = match args.database_type {
        DatabaseKind::Postgresql => {
            let pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(&args.connection_string)
                .await?;
            let db_schema = args.db_schema.as_deref().unwrap_or("public");
            read_schema(&pool, db_schema).await?
        }
        DatabaseKind::Sqlserver => {
            let mut client =
                sqlserver::connect(&args.connection_string, args.trust_server_certificate).await?;
            let db_schema = args.db_schema.as_deref().unwrap_or("dbo");
            read_sqlserver_schema(&mut client, db_schema).await?
        }
    };
    let xml = write_database_xml(&model);

    std::fs::write(&args.file, xml)?;

    println!("Wrote schema to {}", args.file.display());
    Ok(())
}
