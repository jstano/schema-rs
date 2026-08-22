use crate::parse_database_roxml;
use crate::parser::convert::convert_database;
use crate::parser::nodes::DatabaseXml;
use schema_model::model::database_model::DatabaseModel;

/// parse a string containing XML into a DatabaseModel.
pub fn parse_database_xml(xml: &str) -> Result<DatabaseModel, String> {
    let database_xml: DatabaseXml = parse_database_roxml(xml).map_err(|e| format!("XML parse error: {e}"))?;
    convert_database(database_xml)

    // let database_xml: DatabaseXml = qx_from_str(xml).map_err(|e| format!("XML parse error: {e}"))?;
    // Ok(convert_database(database_xml))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_test_resource_schema() {
        let xml = fs::read_to_string("tests/resources/schema-parser-test-schema.xml")
            .expect("resource present");
        let database = parse_database_xml(xml.as_str()).expect("parse ok");
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

    fn wrap(inner: &str) -> String {
        format!(
            r#"<database xmlns="http://stano.com/database">{}</database>"#,
            inner
        )
    }

    #[test]
    fn dangling_relation_target_returns_error_instead_of_panicking() {
        let xml = wrap(
            r#"
            <table name="Child">
                <columns>
                    <column name="ParentId" type="int" required="true"/>
                </columns>
                <relations>
                    <relation src="ParentId" table="Regoin" column="Id" type="cascade"/>
                </relations>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Regoin"));
    }

    #[test]
    fn reverse_relation_resolves_child_in_a_different_named_schema() {
        // The child ("sales.order_item") lives in a non-default schema and references a
        // parent ("product") in the default schema. The reverse relation attached to
        // "product" must be able to resolve "sales.order_item" back - not just "order_item"
        // unqualified, which would incorrectly look for it in the default schema.
        let xml = wrap(
            r#"
            <table name="Product">
                <columns>
                    <column name="Id" type="sequence" required="true"/>
                </columns>
                <keys>
                    <primary>
                        <column name="Id"/>
                    </primary>
                </keys>
            </table>
            <schema name="sales">
                <table name="OrderItem">
                    <columns>
                        <column name="ProductId" type="int" required="true"/>
                    </columns>
                    <relations>
                        <relation src="ProductId" table="Product" column="Id" type="cascade"/>
                    </relations>
                </table>
            </schema>
            "#,
        );

        let database = parse_database_xml(&xml).expect("parse ok");

        let default_schema = database.schemas().iter().find(|s| s.schema_name().is_none()).unwrap();
        let product = default_schema.get_table("Product");
        let reverse_relations = product.reverse_relations();
        assert_eq!(reverse_relations.len(), 1);
        assert_eq!(reverse_relations[0].from_table_name(), "sales.OrderItem");
    }

    #[test]
    fn function_with_unrecognized_database_type_returns_error_instead_of_vanishing() {
        let xml = wrap(
            r#"
            <function name="my_func">
                <sql databaseType="postgres">select 1</sql>
            </function>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("my_func"));
        assert!(err.contains("postgres"));
    }

    #[test]
    fn constraint_with_missing_database_type_returns_error_instead_of_vanishing() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="Id" type="int"/>
                </columns>
                <constraints>
                    <constraint name="ck_id">check (Id > 0)</constraint>
                </constraints>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ck_id"));
    }

    #[test]
    fn view_with_unrecognized_database_type_returns_error_instead_of_silently_applying_to_all() {
        let xml = wrap(
            r#"
            <view name="my_view" databaseType="postgres">select 1</view>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("my_view"));
        assert!(err.contains("postgres"));
    }

    #[test]
    fn view_with_no_database_type_still_parses_and_applies_to_every_database() {
        let xml = wrap(
            r#"
            <view name="my_view">select 1</view>
            "#,
        );

        let database = parse_database_xml(&xml).expect("parse ok");
        let view = &database.default_schema().all_views()[0];
        assert_eq!(view.database_type(), None);
    }

    #[test]
    fn missing_required_table_name_returns_error_instead_of_defaulting_to_empty_string() {
        let xml = wrap(
            r#"
            <table>
                <columns>
                    <column name="Id" type="int"/>
                </columns>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("table"));
        assert!(err.contains("name"));
    }

    #[test]
    fn unrecognized_relation_type_returns_error_instead_of_defaulting_to_enforce() {
        let xml = wrap(
            r#"
            <table name="Child">
                <columns>
                    <column name="ParentId" type="int" required="true"/>
                </columns>
                <relations>
                    <relation src="ParentId" table="Parent" column="Id" type="cascde"/>
                </relations>
            </table>
            <table name="Parent">
                <columns>
                    <column name="Id" type="int" required="true"/>
                </columns>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("cascde"));
    }

    #[test]
    fn unrecognized_lock_escalation_returns_error_instead_of_defaulting_to_auto() {
        let xml = wrap(
            r#"
            <table name="Widget" lockEscalation="sometimes">
                <columns>
                    <column name="Id" type="int"/>
                </columns>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sometimes"));
    }

    #[test]
    fn unrecognized_other_sql_order_returns_error_instead_of_defaulting_to_top() {
        let xml = wrap(
            r#"
            <otherSql databaseType="postgresql" order="middle">select 1</otherSql>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("middle"));
    }

    #[test]
    fn unrecognized_aggregation_frequency_returns_error_instead_of_defaulting_to_monthly() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="Id" type="int"/>
                    <column name="Amount" type="int"/>
                </columns>
                <aggregations>
                    <aggregate destinationTable="WidgetAgg" dateColumn="Id" timestampColumn="Id" frequency="fortnightly">
                        <sum sourceColumn="Amount" destinationColumn="TotalAmount"/>
                        <group>
                            <column source="Id" destination="Id"/>
                        </group>
                    </aggregate>
                </aggregations>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fortnightly"));
    }

    #[test]
    fn invalid_column_type_returns_error_instead_of_panicking() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="Name" type="varchr"/>
                </columns>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Widget"));
        assert!(err.contains("Name"));
    }
}
