use clap::{Arg, Command};
use schema_diff::{ChangeSet, SchemaDiffEngine};
use schema_model::builder::SchemaBuilder;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::schema::Schema;
use schema_model::model::types::DatabaseType;
use schema_parser::parse_database_xml;
use std::fs;

use schema_migration_generator::create_generator;
use schema_migration_generator::source::{read_file_against_head, read_schema_source};

pub fn main() {
    let arguments = Command::new("schema-migration-generator")
        .version("1.0")
        .author("Jeff Stano <jeff@stano.com>")
        .about("Generates a SQL migration from the differences between two database schemas")
        .arg_required_else_help(true)
        .arg(
            Arg::new("database-type")
                .long("database-type")
                .value_name("TYPE")
                .value_parser(["postgresql", "sqlite", "sqlserver"])
                .required(true)
                .ignore_case(true)
                .help("Sets the database type"),
        )
        .arg(
            Arg::new("old")
                .long("old")
                .value_name("SOURCE")
                .required_unless_present("file")
                .conflicts_with("file")
                .help("Sets the old (current) schema source: a file path, or a git '<rev>:<path>' reference (e.g. HEAD:schema.xml)"),
        )
        .arg(
            Arg::new("new")
                .long("new")
                .value_name("SOURCE")
                .required_unless_present("file")
                .conflicts_with("file")
                .help("Sets the new (target) schema source: a file path, or a git '<rev>:<path>' reference (e.g. HEAD:schema.xml)"),
        )
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("FILE")
                .conflicts_with_all(["old", "new"])
                .help("Shortcut: diffs FILE on disk against its latest committed version (HEAD:FILE); equivalent to --old HEAD:FILE --new FILE"),
        )
        .arg(
            Arg::new("output-file")
                .long("output-file")
                .value_name("FILE")
                .help("Sets the output SQL migration file path (default: stdout)"),
        )
        .get_matches();

    let database_type_str = arguments
        .get_one::<String>("database-type")
        .expect("required argument --database-type missing");
    let ((old_label, old_contents), (new_label, new_contents)) = match arguments.get_one::<String>("file") {
        Some(file) => {
            let (old, new) = read_file_against_head(file);
            ((format!("HEAD:{}", file), old), (file.clone(), new))
        }
        None => {
            let old_source = arguments.get_one::<String>("old").expect("required argument --old missing");
            let new_source = arguments.get_one::<String>("new").expect("required argument --new missing");
            ((old_source.clone(), read_schema_source(old_source)), (new_source.clone(), read_schema_source(new_source)))
        }
    };

    let database_type = parse_database_type(database_type_str);
    let old_model = load_schema(&old_label, old_contents);
    let new_model = load_schema(&new_label, new_contents);

    let change_set = diff_models(&old_model, &new_model);

    let generator = create_generator(database_type);
    let mut output = Vec::new();
    generator
        .generate(&change_set, &mut output)
        .expect("failed to generate migration");

    match arguments.get_one::<String>("output-file") {
        Some(output_file) => {
            fs::write(output_file, &output).expect("failed to write output file");
            println!("Wrote migration to {}", output_file);
        }
        None => {
            print!("{}", String::from_utf8_lossy(&output));
        }
    }
}

fn parse_database_type(value: &str) -> DatabaseType {
    match value.to_lowercase().as_str() {
        "postgresql" => DatabaseType::Postgresql,
        "sqlite" => DatabaseType::Sqlite,
        "sqlserver" => DatabaseType::SqlServer,
        other => panic!("Unsupported database type: {}", other),
    }
}

fn load_schema(source: &str, contents: String) -> DatabaseModel {
    let database_model = parse_database_xml(contents.as_str())
        .unwrap_or_else(|err| panic!("failed to parse the schema '{}': {}", source, err));

    let errors = database_model.validate();
    if !errors.is_empty() {
        eprintln!("Error: the schema '{}' is invalid:", source);
        for error in &errors {
            eprintln!("  {}", error);
        }
        std::process::exit(1);
    }

    database_model
}

fn diff_models(old_model: &DatabaseModel, new_model: &DatabaseModel) -> ChangeSet {
    let mut change_set = ChangeSet::new();

    let mut schema_names: Vec<Option<&str>> = old_model.schemas().iter().map(Schema::schema_name).collect();
    for name in new_model.schemas().iter().map(Schema::schema_name) {
        if !schema_names.contains(&name) {
            schema_names.push(name);
        }
    }

    for schema_name in schema_names {
        let empty_schema = SchemaBuilder::new(schema_name).build();
        let old_schema = find_schema(old_model, schema_name).unwrap_or(&empty_schema);
        let new_schema = find_schema(new_model, schema_name).unwrap_or(&empty_schema);
        for change in SchemaDiffEngine::diff(old_schema, new_schema).changes() {
            change_set.add_change(change.clone());
        }
    }

    change_set
}

fn find_schema<'a>(model: &'a DatabaseModel, schema_name: Option<&str>) -> Option<&'a Schema> {
    model.schemas().iter().find(|s| s.schema_name() == schema_name)
}
