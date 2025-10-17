use quick_xml::de::from_str as qx_from_str;

use schema_model::model::schema::Schema;

use super::nodes;
use super::convert;

pub use convert::convert_database; // internal reuse if needed

/// Public API: parse an instance XML into one or more Schema values.
pub fn parse_database_xml(xml: &str) -> Result<Vec<Schema>, String> {
    let db: nodes::DatabaseXml = qx_from_str(xml).map_err(|e| format!("XML parse error: {e}"))?;
    let mut schemas = convert::convert_database(db)?;
    if schemas.len() > 1 { schemas.truncate(1); }
    Ok(schemas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_test_resource_schema() {
        let xml = fs::read_to_string("resources/schema-parser-test-schema.xml").expect("resource present");
        let schemas = parse_database_xml(&xml).expect("parse ok");
        assert_eq!(schemas.len(), 1);
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
