use clap::Parser;
use schema_reverse_engineer::{read_schema, write_database_xml};
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "schema-reverse-engineer")]
#[command(about = "Reverse-engineer a live PostgreSQL database into a schema-rs XML schema definition")]
struct Args {
    #[arg(long, help = "PostgreSQL connection string, e.g. postgres://user:pass@host:5432/dbname")]
    connection_string: String,

    #[arg(long, default_value = "public", help = "Postgres schema to introspect")]
    db_schema: String,

    #[arg(long, help = "Path to write the generated XML schema file to")]
    file: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&args.connection_string)
        .await?;

    let model = read_schema(&pool, &args.db_schema).await?;
    let xml = write_database_xml(&model);

    std::fs::write(&args.file, xml)?;

    println!("Wrote schema to {}", args.file.display());
    Ok(())
}
