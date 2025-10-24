use quick_xml::de::from_str as qx_from_str;

use super::convert;
use super::nodes;

pub use convert::convert_database;
use schema_model::model::database_model::DatabaseModel;

/// parse an instance XML into a Database with one or more Schema values.
pub fn parse_database_xml(xml: &str) -> Result<DatabaseModel, String> {
    let db: nodes::DatabaseXml = qx_from_str(xml).map_err(|e| format!("XML parse error: {e}"))?;
    Ok(convert_database(db)?)
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
        let s = &schemas[0];
        // Expect at least 3 tables shown in the sample
        assert!(s.tables().len() >= 3);

        let parent = s.get_table("ParentTable");
        assert_eq!(parent.columns().len(), 4);
        assert!(parent.primary_key().is_some());

        let child = s.get_table("ChildTable");
        assert_eq!(child.columns().len(), 3);
        assert!(child.primary_key().is_some());

        let tester = s.get_table("ColumnTesterTable");
        assert!(tester.has_column("varchar"));
        assert!(tester.has_column("sequence"));
    }
}
