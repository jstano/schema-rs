use std::fs;
use std::path::Path;
use clap::{Arg, Command};
use schema_model::model::database_model::DatabaseModel;
use schema_parser::parse_database_xml;
use schema_diagram_generator::common::generate_options::DiagramGenerateOptions;
use schema_diagram_generator::common::generator_format::DiagramFormat;
use std::rc::Rc;

pub fn main() {
    let arguments = Command::new("schema-diagram-generator")
        .version("1.0")
        .author("Jeff Stano <jeff@stano.com>")
        .about("Generates ER diagrams from database schemas")
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .value_parser(["mermaid", "plantuml"])
                .required(true)
                .ignore_case(true)
                .help("Sets the diagram format (mermaid or plantuml)"),
        )
        .arg(
            Arg::new("schema-file")
                .long("schema-file")
                .value_name("FILE")
                .required(true)
                .help("Sets the schema file location"),
        )
        .get_matches();

    let format_str = arguments
        .get_one::<String>("format")
        .expect("required argument --format missing");
    let schema_file = arguments
        .get_one::<String>("schema-file")
        .expect("required argument --schema-file missing");

    let schema_path = Path::new(schema_file);
    let diagram_format: DiagramFormat = format_str.parse().expect("Invalid diagram format");
    let output_path = build_output_path(schema_path, diagram_format.format_name());

    let database_model = load_schema(schema_path);
    let options = DiagramGenerateOptions {
        database_model: Rc::new(database_model),
    };

    let output = diagram_format.generate(options);
    fs::write(&output_path, &output).expect("Failed to write output file");
    println!("{}", output_path);
}

fn load_schema(schema_path: &Path) -> DatabaseModel {
    let contents = fs::read_to_string(schema_path).expect("failed to read the schema file");
    parse_database_xml(contents.as_str()).expect("failed to parse the schema")
}

fn build_output_path(path: &Path, format_name: &str) -> String {
    let parent = path.parent().expect("Path has no parent");
    let stem = path.file_stem().expect("No file stem").to_string_lossy();
    let new_filename = format!("{}-{}.md", stem, format_name);
    parent.join(new_filename).to_string_lossy().to_string()
}
