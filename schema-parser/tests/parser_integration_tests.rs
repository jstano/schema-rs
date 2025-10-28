use schema_parser::parse_database_xml;
use std::fs;

#[test]
fn test_parser() {
    let xml =
        fs::read_to_string("resources/schema-parser-test-schema.xml").expect("resource present");
    let database = parse_database_xml(xml.as_str()).expect("parse ok");
    let schemas = database.schemas();

    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].tables().len(), 7);
    assert_eq!(schemas[0].get_table("ParentTable").name(), "ParentTable");
    assert_eq!(schemas[0].get_table("ChildTable").name(), "ChildTable");
    assert_eq!(schemas[0].get_table("ColumnTesterTable").name(), "ColumnTesterTable");
    assert_eq!(schemas[0].get_table("Property").name(), "Property");
    assert_eq!(schemas[0].get_table("Region").name(), "Region");
    assert_eq!(schemas[0].get_table("KBI").name(), "KBI");
    assert_eq!(schemas[0].get_table("MasterKBICode").name(), "MasterKBICode");
    assert_eq!(schemas[1].schema_name().unwrap(), "test");
    assert_eq!(schemas[1].tables().len(), 1);
    assert_eq!(schemas[1].get_table("Unit").name(), "Unit");
}
