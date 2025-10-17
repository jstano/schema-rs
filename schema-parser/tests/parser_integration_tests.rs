use std::fs;
use schema_parser::parse_database_xml;

#[test]
fn test_parser() {
    let xml = fs::read_to_string("resources/schema-parser-test-schema.xml").expect("resource present");
    let schemas = parse_database_xml(&xml).expect("parse ok");

    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].get_table("ParentTable").name(), "ParentTable");
}
