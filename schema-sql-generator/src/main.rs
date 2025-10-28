use std::cell::RefCell;
use std::{env, fs};
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use clap::{Arg, Command};
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use schema_parser::parse_database_xml;
use schema_sql_generator::common::generate_options::GenerateOptions;
use schema_sql_generator::common::generator_type::{GeneratorType};
use schema_sql_generator::common::output_mode::OutputMode;
use schema_sql_generator::common::print_writer::PrintWriter;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    println!("Program arguments: {:?}", args);

    let arguments = Command::new("schema")
        .version("1.0")
        .author("Jeff Stano <jeff@stano.com>")
        .about("Manages database schemas")
        .arg(Arg::new("database-type")
            .long("database-type")
            .value_name("TYPE")
            .value_parser(["h2", "postgres", "mysql", "sqlite", "sqlserver"])
            .required(true)
            .num_args(1)
            .ignore_case(true)
            .help("Sets the database type"))
        .arg(Arg::new("schema-file")
            .long("schema-file")
            .value_name("FILE")
            .required(true)
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
        .get_matches();

    let empty = String::new();
    let database_type = arguments.get_one::<String>("database-type").expect("required argument --database-type missing");
    let schema_file = arguments.get_one::<String>("schema-file").expect("required argument --schema-file missing");
    let foreign_key_mode = arguments.get_one::<String>("foreign-key-mode").unwrap_or(&empty);
    let boolean_mode = arguments.get_one::<String>("boolean-mode").unwrap_or(&empty);
    let output_mode = arguments.get_one::<String>("output-mode").unwrap_or(&empty);
    let schema_path = Path::new(schema_file);
    let output_path = schema_path.with_extension("sql");
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
    };

    generator_type.generate(options);
}

fn load_schema(schema_path: &Path) -> DatabaseModel {
    let contents = fs::read_to_string(schema_path).expect("failed to read the schema file");
    parse_database_xml(contents.as_str()).expect("failed to parse the schema")
}
