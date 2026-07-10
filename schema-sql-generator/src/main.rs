use clap::{Arg, ArgAction, Command};
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use schema_parser::parse_database_xml;
use schema_sql_generator::common::generate_options::GenerateOptions;
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_sql_generator::common::output_mode::OutputMode;
use schema_sql_generator::common::print_writer::PrintWriter;
use std::cell::RefCell;
use std::fs::{self, File};
use std::path::Path;
use std::rc::Rc;

const EMPTY_SCHEMA_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://stano.com/database"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://stano.com/database http://schema.stano.com/schema.xsd">
</database>
"#;

pub fn main() {
    let mut cmd = Command::new("schema")
        .version("1.0")
        .author("Jeff Stano <jeff@stano.com>")
        .about("Manages database schemas")
        .arg(Arg::new("database-type")
            .long("database-type")
            .value_name("TYPE")
            .value_parser(["postgresql", "sqlite", "sqlserver"])
            .required(false)
            .num_args(1)
            .ignore_case(true)
            .help("Sets the database type"))
        .arg(Arg::new("schema-file")
            .long("schema-file")
            .value_name("FILE")
            .required(false)
            .help("Sets the schema file location"))
        .arg(Arg::new("foreign-key-mode")
            .long("foreign-key-mode")
            .value_name("MODE")
            .value_parser(["none", "relations", "triggers"])
            .help("Sets the foreign key mode"))
        .arg(Arg::new("boolean-mode")
            .long("boolean-mode")
            .value_name("MODE")
            .value_parser(["native", "yesno", "yn"])
            .help("Sets the boolean mode"))
        .arg(Arg::new("output-mode")
            .long("output-mode")
            .value_name("MODE")
            .value_parser(["all", "indexes-only", "triggers-only"])
            .help("Sets the output mode"))
        .arg(Arg::new("postgresql-version")
            .long("postgresql-version")
            .value_name("VERSION")
            .help("Target PostgreSQL major version (e.g. 17, 18); affects UUID default function"))
        .arg(Arg::new("new-schema")
            .long("new-schema")
            .action(ArgAction::SetTrue)
            .help("Generate a new empty schema.xml file (requires --schema-file or defaults to ./schema.xml)"));

    let arguments = cmd.clone().get_matches();

    if arguments.get_flag("new-schema") {
        let schema_file = arguments
            .get_one::<String>("schema-file")
            .map(|s| s.as_str())
            .unwrap_or("schema.xml");

        if Path::new(schema_file).exists() {
            eprintln!("Error: file '{}' already exists. Use a different path or delete the existing file.", schema_file);
            std::process::exit(1);
        }

        fs::write(schema_file, EMPTY_SCHEMA_TEMPLATE)
            .expect("Failed to write schema file");
        println!("Created empty schema file: {}", schema_file);
        return;
    }

    let empty = String::new();
    let database_type = match arguments.get_one::<String>("database-type") {
        Some(db) => db,
        None => {
            eprintln!("Error: --database-type is required unless --new-schema is set");
            cmd.print_help().ok();
            std::process::exit(1);
        }
    };
    let schema_file = match arguments.get_one::<String>("schema-file") {
        Some(file) => file,
        None => {
            eprintln!("Error: --schema-file is required unless --new-schema is set");
            cmd.print_help().ok();
            std::process::exit(1);
        }
    };
    let foreign_key_mode = arguments.get_one::<String>("foreign-key-mode").unwrap_or(&empty);
    let boolean_mode = arguments.get_one::<String>("boolean-mode").unwrap_or(&empty);
    let output_mode = arguments.get_one::<String>("output-mode").unwrap_or(&empty);
    let target_postgres_version: u32 = arguments
        .get_one::<String>("postgresql-version")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let schema_path = Path::new(schema_file);
    let output_path = build_output_path(schema_path, database_type.to_string().to_lowercase());
    let output_file = File::create(output_path).expect("");
    let print_writer = PrintWriter::new(Box::new(output_file));
    let generator_type: GeneratorType = database_type.parse().unwrap();
    let database_model = load_schema(schema_path);
    let options = GenerateOptions {
        database_model: Rc::new(database_model),
        writer: Rc::new(RefCell::new(print_writer)),
        boolean_mode: boolean_mode.parse().unwrap_or(BooleanMode::Native),
        foreign_key_mode: foreign_key_mode.parse().unwrap_or(ForeignKeyMode::Relations),
        output_mode: output_mode.parse().unwrap_or(OutputMode::All),
        target_postgres_version,
    };

    generator_type.generate(options);
}

fn load_schema(schema_path: &Path) -> DatabaseModel {
    let contents = fs::read_to_string(schema_path).expect("failed to read the schema file");
    parse_database_xml(contents.as_str()).expect("failed to parse the schema")
}

fn build_output_path(path: &Path, database_type: String) -> String {
    let parent = path.parent().expect("Path has no parent");
    let stem = path.file_stem().expect("No file stem").to_string_lossy();
    let new_filename = format!("{}-{}.sql", stem, database_type);

    parent.join(new_filename).to_string_lossy().to_string()
}
