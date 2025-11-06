use schema_model::model::database_model::DatabaseModel;
use crate::parse_database_roxml;
use crate::parser::convert::convert_database;
use crate::parser::nodes::DatabaseXml;

/// parse a string containing XML into a DatabaseModel.
pub fn parse_database_xml(xml: &str) -> Result<DatabaseModel, String> {
    let database_xml: DatabaseXml = parse_database_roxml(xml).map_err(|e| format!("XML parse error: {e}"))?;
    Ok(convert_database(database_xml))

    // let database_xml: DatabaseXml = qx_from_str(xml).map_err(|e| format!("XML parse error: {e}"))?;
    // Ok(convert_database(database_xml))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_test_resource_schema() {
        let xml = fs::read_to_string("resources/schema-parser-test-schema.xml")
            .expect("resource present");
        let database = parse_database_xml(xml.as_str()).expect("parse ok");
        let version = database.version().unwrap();
        assert_eq!(version.major_version(), 1);
        assert_eq!(version.minor_version(), 2);
        let schemas = database.schemas();
        assert_eq!(schemas.len(), 2);
        let default_schema = &schemas[0];
        // Expect at least 3 tables shown in the sample
        assert!(default_schema.tables().len() >= 3);

        let parent = default_schema.get_table("ParentTable");
        assert_eq!(parent.columns().len(), 4);
        assert!(parent.primary_key().is_some());

        let child = default_schema.get_table("ChildTable");
        assert_eq!(child.columns().len(), 3);
        assert!(child.primary_key().is_some());

        let tester = default_schema.get_table("ColumnTesterTable");
        assert!(tester.has_column("varchar"));
        assert!(tester.has_column("sequence"));
    }
}
