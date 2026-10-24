use crate::parse_database_roxml;
use crate::parser::convert::convert_database;
use crate::parser::nodes::DatabaseXml;
use schema_model::model::database_model::DatabaseModel;

/// parse a string containing XML into a DatabaseModel.
pub fn parse_database_xml(xml: &str) -> Result<DatabaseModel, String> {
    let database_xml: DatabaseXml =
        parse_database_roxml(xml).map_err(|e| format!("XML parse error: {e}"))?;
    convert_database(database_xml)

    // let database_xml: DatabaseXml = qx_from_str(xml).map_err(|e| format!("XML parse error: {e}"))?;
    // Ok(convert_database(database_xml))
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema_model::model::aggregation::AggregationType;
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
    fn database_specific_checks_are_kept_per_database() {
        use schema_model::model::types::DatabaseType;
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="Qty" type="int">
                        <check>Qty &gt; 0</check>
                        <check databaseType="postgresql">Qty &gt; 100</check>
                        <check databaseType="h2">Qty &gt; 999</check>
                    </column>
                </columns>
            </table>"#,
        );
        let database = parse_database_xml(&xml).expect("parse ok");
        let qty = database
            .default_schema()
            .get_table("Widget")
            .column("Qty")
            .clone();

        assert_eq!(qty.check_constraint(), Some("Qty > 0"));
        assert_eq!(
            qty.check_constraint_for(DatabaseType::Postgresql),
            Some("Qty > 100")
        );
        assert_eq!(
            qty.check_constraint_for(DatabaseType::SqlServer),
            Some("Qty > 0")
        );
        // h2 is not a database this library generates for, so that check is dropped.
        assert_eq!(qty.database_checks().len(), 1);
    }

    fn parse_err(inner: &str) -> String {
        match parse_database_xml(&wrap(inner)) {
            Ok(_) => panic!("expected a parse error"),
            Err(e) => e,
        }
    }

    #[test]
    fn h2_targeted_elements_are_accepted_and_dropped() {
        use schema_model::model::types::DatabaseType;
        let xml = wrap(
            r#"
            <table name="T">
                <columns><column name="A" type="int"><check databaseType="h2">A &gt; 0</check></column></columns>
                <triggers>
                    <update databaseType="h2">select 1;</update>
                    <update databaseType="sqlite">select 2;</update>
                </triggers>
                <constraints>
                    <constraint name="CK_H2" databaseType="h2">A &lt; 9</constraint>
                    <constraint name="CK_PG" databaseType="postgresql">A &lt; 9</constraint>
                </constraints>
                <initialData>
                    <sql databaseType="h2">insert into t values (1);</sql>
                    <sql>insert into t values (2);</sql>
                </initialData>
            </table>
            <view name="V" databaseType="h2">select 1</view>
            <view name="W">select 1</view>
            <function name="F"><sql databaseType="h2">x</sql></function>
            <procedure name="P"><sql databaseType="h2">x</sql><sql databaseType="sqlite">y</sql></procedure>
            <otherSql databaseType="h2" order="top">select 1</otherSql>"#,
        );
        let database = parse_database_xml(&xml).expect("h2 elements parse");
        let schema = database.default_schema();
        let table = schema.get_table("T");
        assert_eq!(table.triggers().len(), 1);
        assert_eq!(table.constraints().len(), 1);
        assert_eq!(table.initial_data().len(), 1);
        assert_eq!(schema.views(DatabaseType::Postgresql).len(), 1);
        assert!(schema.functions().is_empty());
        assert_eq!(schema.procedures().len(), 1);
        assert!(schema.other_sql().is_empty());
    }

    #[test]
    fn constraint_without_database_type_applies_to_every_database() {
        let xml = wrap(
            r#"<table name="T"><columns><column name="A" type="int"/></columns>
            <constraints><constraint name="CK">A &gt; 0</constraint></constraints></table>"#,
        );
        let database = parse_database_xml(&xml).expect("parse ok");
        let constraints = database
            .default_schema()
            .get_table("T")
            .constraints()
            .to_vec();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].database_type(), None);
    }

    #[test]
    fn unknown_database_type_is_still_rejected() {
        let err = parse_err(
            r#"<table name="T"><columns><column name="A" type="int"/></columns><constraints><constraint name="C" databaseType="oracle">x</constraint></constraints></table>"#,
        );
        assert!(err.contains("unrecognized databaseType 'oracle'"), "{err}");
        let err = parse_err(
            r#"<table name="T"><columns><column name="A" type="int"><check databaseType="oracle">x</check></column></columns></table>"#,
        );
        assert!(err.contains("unrecognized databaseType 'oracle'"), "{err}");
    }

    #[test]
    fn negative_scale_or_length_is_rejected() {
        let err = parse_err(
            r#"<table name="T"><columns><column name="A" type="decimal" length="10" scale="-2"/></columns></table>"#,
        );
        assert!(err.contains("scale") && err.contains("negative"), "{err}");
        let err = parse_err(
            r#"<table name="T"><columns><column name="A" type="varchar" length="-1"/></columns></table>"#,
        );
        assert!(err.contains("length") && err.contains("negative"), "{err}");
    }

    #[test]
    fn primary_key_where_is_rejected() {
        let err = parse_err(
            r#"<table name="T"><columns><column name="ID" type="int"/></columns><keys><primary where="ID &gt; 0"><column name="ID"/></primary></keys></table>"#,
        );
        assert!(err.contains("<primary>") && err.contains("where"), "{err}");
    }

    #[test]
    fn case_sensitive_text_applies_to_every_schema_and_is_rejected_on_schema() {
        let xml = r#"<database xmlns="http://stano.com/database" caseSensitiveText="false">
            <schema name="s"><table name="T"><columns><column name="A" type="int"/></columns></table></schema>
        </database>"#;
        let database = parse_database_xml(xml).expect("parse ok");
        assert!(!database.schemas().iter().any(|s| s.case_sensitive_text()));

        let err = parse_err(r#"<schema name="s" caseSensitiveText="false"/>"#);
        assert!(err.contains("caseSensitiveText"), "{err}");
    }

    #[test]
    fn empty_key_is_rejected() {
        let err = parse_err(
            r#"<table name="T"><columns><column name="ID" type="int"/></columns><keys><primary/></keys></table>"#,
        );
        assert!(err.contains("at least one <column>"), "{err}");
    }

    #[test]
    fn duplicate_column_checks_are_rejected() {
        let err = parse_err(
            r#"<table name="T"><columns><column name="ID" type="int"><check databaseType="h2">ID &gt; 0</check><check databaseType="h2">ID &gt; 1</check></column></columns></table>"#,
        );
        assert!(
            err.contains("more than one check") && err.contains("h2"),
            "{err}"
        );
        let err = parse_err(
            r#"<table name="T"><columns><column name="ID" type="int"><check>ID &gt; 0</check><check>ID &gt; 1</check></column></columns></table>"#,
        );
        assert!(err.contains("more than one check"), "{err}");
    }

    #[test]
    fn unknown_elements_are_rejected() {
        let err = parse_err("<bogus/>");
        assert!(
            err.contains("<bogus>") && err.contains("<database>"),
            "{err}"
        );
        let err = parse_err(
            r#"<table name="T"><columns><column name="ID" type="int"/></columns><bogus/></table>"#,
        );
        assert!(err.contains("<bogus>") && err.contains("<table>"), "{err}");
        let err = parse_err(
            r#"<table name="T"><columns><column name="ID" type="int"><bogus/></column></columns></table>"#,
        );
        assert!(err.contains("<bogus>") && err.contains("<column>"), "{err}");
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

        let default_schema = database
            .schemas()
            .iter()
            .find(|s| s.schema_name().is_none())
            .unwrap();
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

    #[test]
    fn primary_key_with_where_predicate_returns_error() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="Id" type="int" required="true"/>
                </columns>
                <keys>
                    <primary where="1=1">
                        <column name="Id"/>
                    </primary>
                </keys>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("primary"));
    }

    #[test]
    fn clustered_unique_key_with_where_predicate_returns_error() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="ParentId" type="int"/>
                </columns>
                <keys>
                    <unique cluster="true" where="ParentId is not null">
                        <column name="ParentId"/>
                    </unique>
                </keys>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("clustered"));
    }

    #[test]
    fn unique_key_with_where_predicate_is_promoted_to_a_filtered_unique_index() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="ParentId" type="int"/>
                </columns>
                <keys>
                    <unique where="ParentId is not null">
                        <column name="ParentId"/>
                    </unique>
                </keys>
            </table>
            "#,
        );

        let database = parse_database_xml(&xml).expect("parse ok");
        let schema = &database.schemas()[0];
        let table = schema.get_table("Widget");

        assert!(table.keys().is_empty());
        assert_eq!(table.indexes().len(), 1);
        let index = &table.indexes()[0];
        assert!(index.is_unique());
        assert_eq!(index.filter(), Some("ParentId is not null"));
    }

    #[test]
    fn index_where_predicate_is_parsed() {
        let xml = wrap(
            r#"
            <table name="Widget">
                <columns>
                    <column name="ParentId" type="int"/>
                </columns>
                <keys>
                    <index unique="true" where="ParentId is not null">
                        <column name="ParentId"/>
                    </index>
                </keys>
            </table>
            "#,
        );

        let database = parse_database_xml(&xml).expect("parse ok");
        let schema = &database.schemas()[0];
        let table = schema.get_table("Widget");

        assert_eq!(table.indexes().len(), 1);
        let index = &table.indexes()[0];
        assert!(index.is_unique());
        assert_eq!(index.filter(), Some("ParentId is not null"));
    }

    #[test]
    fn aggregate_columns_keep_document_order() {
        let xml = wrap(
            r#"
            <table name="T">
                <columns><column name="ID" type="int"/></columns>
                <aggregations>
                    <aggregate destinationTable="D" dateColumn="Day" timestampColumn="Stamp" frequency="daily">
                        <sum sourceColumn="ID" destinationColumn="S1"/>
                        <count destinationColumn="C1"/>
                        <sum sourceColumn="ID" destinationColumn="S2"/>
                        <group><column source="ID" destination="ID"/></group>
                    </aggregate>
                </aggregations>
            </table>
            "#,
        );

        let database = parse_database_xml(&xml).expect("parse ok");
        let table = database.schemas()[0].get_table("T");
        let columns = table.aggregations()[0].aggregation_columns();

        let kinds: Vec<_> = columns
            .iter()
            .map(|c| (c.aggregation_type(), c.destination_column()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (AggregationType::Sum, "S1"),
                (AggregationType::Count, "C1"),
                (AggregationType::Sum, "S2"),
            ]
        );
    }

    #[test]
    fn aggregate_with_multiple_groups_returns_error() {
        let xml = wrap(
            r#"
            <table name="T">
                <columns><column name="ID" type="int"/></columns>
                <aggregations>
                    <aggregate destinationTable="D" dateColumn="Day" timestampColumn="Stamp" frequency="daily">
                        <count destinationColumn="C1"/>
                        <group><column source="ID" destination="ID"/></group>
                        <group><column source="ID" destination="ID"/></group>
                    </aggregate>
                </aggregations>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.unwrap_err().contains("more than one <group>"));
    }

    #[test]
    fn aggregate_without_group_returns_error() {
        let xml = wrap(
            r#"
            <table name="T">
                <columns><column name="ID" type="int"/></columns>
                <aggregations>
                    <aggregate destinationTable="D" dateColumn="Day" timestampColumn="Stamp" frequency="daily">
                        <count destinationColumn="C1"/>
                    </aggregate>
                </aggregations>
            </table>
            "#,
        );

        let result = parse_database_xml(&xml);

        assert!(result.unwrap_err().contains("missing <group>"));
    }
}
