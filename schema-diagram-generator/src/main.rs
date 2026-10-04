use clap::{Arg, Command};
use schema_diagram_generator::common::generate_options::DiagramGenerateOptions;
use schema_diagram_generator::common::generator_format::DiagramFormat;
use schema_model::model::database_model::DatabaseModel;
use schema_parser::parse_database_xml;
use std::fs;
use std::path::Path;
use std::rc::Rc;

pub fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Command::new("schema-diagram-generator")
        .version("1.0")
        .author("Jeff Stano <jeff@stano.com>")
        .about("Generates ER diagrams from database schemas")
        .arg_required_else_help(true)
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
        .arg(
            Arg::new("output-file")
                .long("output-file")
                .value_name("FILE")
                .help("Sets the exact output file path (default: {schema-stem}.{mmd|puml} next to --schema-file)"),
        )
        .get_matches();

    let format_str = arguments
        .get_one::<String>("format")
        .ok_or("--format is required")?;
    let schema_file = arguments
        .get_one::<String>("schema-file")
        .ok_or("--schema-file is required")?;

    let schema_path = Path::new(schema_file);
    let diagram_format: DiagramFormat = format_str.parse()?;
    let output_path = match arguments.get_one::<String>("output-file") {
        Some(path) => path.clone(),
        None => build_output_path(schema_path, diagram_format.file_extension())?,
    };

    let database_model = load_schema(schema_path)?;
    let options = DiagramGenerateOptions {
        database_model: Rc::new(database_model),
    };

    let output = diagram_format.generate(options);
    if let Some(parent) = Path::new(&output_path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create output directory '{}': {}",
                parent.display(),
                e
            )
        })?;
    }
    fs::write(&output_path, &output)
        .map_err(|e| format!("failed to write output file '{}': {}", output_path, e))?;
    println!("{}", output_path);
    Ok(())
}

fn load_schema(schema_path: &Path) -> Result<DatabaseModel, String> {
    let contents = fs::read_to_string(schema_path).map_err(|e| {
        format!(
            "failed to read the schema file '{}': {}",
            schema_path.display(),
            e
        )
    })?;
    parse_database_xml(contents.as_str()).map_err(|e| {
        format!(
            "failed to parse the schema '{}': {}",
            schema_path.display(),
            e
        )
    })
}

fn build_output_path(path: &Path, extension: &str) -> Result<String, String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "schema file path '{}' has no parent directory",
            path.display()
        )
    })?;
    let stem = path
        .file_stem()
        .ok_or_else(|| format!("schema file path '{}' has no file name", path.display()))?
        .to_string_lossy();
    let new_filename = format!("{}.{}", stem, extension);
    Ok(parent.join(new_filename).to_string_lossy().to_string())
}
