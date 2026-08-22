use crate::parser::nodes::*;
use roxmltree::{Document, Node};

const NS: &str = "http://stano.com/database";

pub fn parse_database_roxml(xml: &str) -> Result<DatabaseXml, String> {
    let doc = Document::parse(xml).map_err(|e| format!("XML parse error: {e}"))?;
    let root = doc
        .descendants()
        .find(|n| n.has_tag_name((NS, "database")))
        .ok_or_else(|| "No <database> root element found".to_string())?;

    parse_database_node(root)
}

fn parse_database_node(node: Node) -> Result<DatabaseXml, String> {
    let foreign_key_mode = attr_string(node, "foreignKeyMode");
    let boolean_mode = attr_string(node, "booleanMode");
    let case_sensitive_text = attr_bool(node, "caseSensitiveText");

    let mut tables = Vec::new();
    let mut enums = Vec::new();
    let mut views = Vec::new();
    let mut functions = Vec::new();
    let mut procedures = Vec::new();
    let mut other_sql = Vec::new();
    let mut schemas = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "table" if child.has_tag_name((NS, "table")) => tables.push(parse_table_node(child)?),
            "enum" if child.has_tag_name((NS, "enum")) => enums.push(parse_enum_node(child)?),
            "view" if child.has_tag_name((NS, "view")) => views.push(parse_view_node(child)?),
            "function" if child.has_tag_name((NS, "function")) => functions.push(parse_function_node(child)?),
            "procedure" if child.has_tag_name((NS, "procedure")) => procedures.push(parse_procedure_node(child)?),
            "otherSql" if child.has_tag_name((NS, "otherSql")) => other_sql.push(parse_other_sql_node(child)?),
            "schema" if child.has_tag_name((NS, "schema")) => schemas.push(parse_schema_node(child)?),
            _ => {}
        }
    }

    Ok(DatabaseXml {
        foreign_key_mode,
        boolean_mode,
        case_sensitive_text,
        tables,
        enums,
        views,
        functions,
        procedures,
        other_sql,
        schemas,
    })
}

fn parse_schema_node(node: Node) -> Result<SchemaXml, String> {
    let name = attr_string_required(node, "name")?;
    let case_sensitive_text = attr_bool(node, "caseSensitiveText");

    let mut tables = Vec::new();
    let mut enums = Vec::new();
    let mut views = Vec::new();
    let mut functions = Vec::new();
    let mut procedures = Vec::new();
    let mut other_sql = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "table" if child.has_tag_name((NS, "table")) => tables.push(parse_table_node(child)?),
            "enum" if child.has_tag_name((NS, "enum")) => enums.push(parse_enum_node(child)?),
            "view" if child.has_tag_name((NS, "view")) => views.push(parse_view_node(child)?),
            "function" if child.has_tag_name((NS, "function")) => functions.push(parse_function_node(child)?),
            "procedure" if child.has_tag_name((NS, "procedure")) => procedures.push(parse_procedure_node(child)?),
            "otherSql" if child.has_tag_name((NS, "otherSql")) => other_sql.push(parse_other_sql_node(child)?),
            _ => {}
        }
    }

    Ok(SchemaXml { name, case_sensitive_text, tables, enums, views, functions, procedures, other_sql })
}

fn parse_table_node(node: Node) -> Result<TableXml, String> {
    let name = attr_string_required(node, "name")?;
    let data_opt = attr_bool(node, "data");
    let no_export = attr_bool(node, "noExport");
    let export_data_column = attr_string(node, "exportDataColumn");
    let compress = attr_bool(node, "compress");
    let lock_escalation = attr_string(node, "lockEscalation");

    let mut columns: Option<ColumnsXml> = None;
    let mut keys: Option<KeysXml> = None;
    let mut relations: Option<RelationsXml> = None;
    let mut triggers: Option<TriggersXml> = None;
    let mut constraints: Option<ConstraintsXml> = None;
    let mut aggregations: Option<AggregationsXml> = None;
    let mut initial_data: Option<InitialDataXml> = None;

    for child in node.children().filter(|n| n.is_element()) {
        if child.has_tag_name((NS, "columns")) {
            columns = Some(parse_columns_node(child)?);
        } else if child.has_tag_name((NS, "keys")) {
            keys = Some(parse_keys_node(child)?);
        } else if child.has_tag_name((NS, "relations")) {
            relations = Some(parse_relations_node(child)?);
        } else if child.has_tag_name((NS, "triggers")) {
            triggers = Some(parse_triggers_node(child)?);
        } else if child.has_tag_name((NS, "constraints")) {
            constraints = Some(parse_constraints_node(child)?);
        } else if child.has_tag_name((NS, "aggregations")) {
            aggregations = Some(parse_aggregations_node(child)?);
        } else if child.has_tag_name((NS, "initialData")) {
            initial_data = Some(parse_initial_data_node(child));
        }
    }

    Ok(TableXml {
        name,
        data_opt,
        no_export,
        export_data_column,
        compress,
        lock_escalation,
        columns,
        keys,
        relations,
        triggers,
        constraints,
        aggregations,
        initial_data,
    })
}

fn parse_columns_node(node: Node) -> Result<ColumnsXml, String> {
    let mut cols = Vec::new();
    for c in node.children().filter(|n| n.has_tag_name((NS, "column"))) {
        cols.push(parse_column_node(c)?);
    }
    Ok(ColumnsXml { column: cols })
}

fn parse_column_node(node: Node) -> Result<ColumnXml, String> {
    Ok(ColumnXml {
        name: attr_string_required(node, "name")?,
        r#type: attr_string_required(node, "type")?,
        length: attr_i32(node, "length"),
        scale: attr_i32(node, "scale"),
        required: attr_bool(node, "required"),
        default_value: attr_string(node, "default"),
        generated: attr_string(node, "generated"),
        enum_type: attr_string(node, "enumType"),
        element_type: attr_string(node, "elementType"),
        min_value: attr_f64(node, "minValue"),
        max_value: attr_f64(node, "maxValue"),
        check: node
            .children()
            .find(|n| n.has_tag_name((NS, "check")))
            .map(parse_check_node),
    })
}

fn parse_check_node(node: Node) -> CheckXml {
    let text = node.text().map(|s| s.trim()).filter(|s| !s.is_empty());
    CheckXml { value: text.map(|s| s.to_string()) }
}

fn parse_keys_node(node: Node) -> Result<KeysXml, String> {
    let mut primary: Option<KeyColumnsXml> = None;
    let mut uniques: Vec<KeyColumnsXml> = Vec::new();
    let mut indexes: Vec<IndexXml> = Vec::new();

    for c in node.children().filter(|n| n.is_element()) {
        if c.has_tag_name((NS, "primary")) {
            primary = Some(parse_key_columns_node(c)?);
        } else if c.has_tag_name((NS, "unique")) {
            uniques.push(parse_key_columns_node(c)?);
        } else if c.has_tag_name((NS, "index")) {
            indexes.push(parse_index_node(c)?);
        }
    }

    Ok(KeysXml { primary, uniques, indexes })
}

fn parse_key_columns_node(node: Node) -> Result<KeyColumnsXml, String> {
    let mut columns = Vec::new();
    for c in node.children().filter(|n| n.has_tag_name((NS, "column"))) {
        columns.push(KeyColumnXml {
            name: attr_string_required(c, "name")?,
        });
    }
    let cluster = attr_bool(node, "cluster");
    Ok(KeyColumnsXml { columns, cluster })
}

fn parse_index_node(node: Node) -> Result<IndexXml, String> {
    let mut columns = Vec::new();
    for c in node.children().filter(|n| n.has_tag_name((NS, "column"))) {
        columns.push(KeyColumnXml { name: attr_string_required(c, "name")? });
    }
    Ok(IndexXml {
        columns,
        include: attr_string(node, "include"),
        compress: attr_bool(node, "compress"),
        unique: attr_bool(node, "unique"),
    })
}

fn parse_relations_node(node: Node) -> Result<RelationsXml, String> {
    let mut rels = Vec::new();
    for r in node.children().filter(|n| n.has_tag_name((NS, "relation"))) {
        rels.push(RelationXml {
            src: attr_string_required(r, "src")?,
            table: attr_string_required(r, "table")?,
            column: attr_string_required(r, "column")?,
            r#type: attr_string_required(r, "type")?,
            disable_usage_checking: attr_bool(r, "disableUsageChecking"),
        });
    }
    Ok(RelationsXml { relation: rels })
}

fn parse_view_node(node: Node) -> Result<ViewXml, String> {
    Ok(ViewXml {
        name: attr_string_required(node, "name")?,
        database_type: attr_string(node, "databaseType"),
        sql: collect_text(node),
    })
}

fn parse_function_node(node: Node) -> Result<FunctionXml, String> {
    let name = attr_string_required(node, "name")?;
    let mut sql = Vec::new();
    for s in node.children().filter(|n| n.has_tag_name((NS, "sql"))) {
        sql.push(VendorSqlXml {
            database_type: attr_string_required(s, "databaseType")?,
            sql: collect_text(s),
        });
    }
    Ok(FunctionXml { name, sql })
}

fn parse_procedure_node(node: Node) -> Result<ProcedureXml, String> {
    let name = attr_string_required(node, "name")?;
    let mut sql = Vec::new();
    for s in node.children().filter(|n| n.has_tag_name((NS, "sql"))) {
        sql.push(VendorSqlXml {
            database_type: attr_string_required(s, "databaseType")?,
            sql: collect_text(s),
        });
    }
    Ok(ProcedureXml { name, sql })
}

fn parse_other_sql_node(node: Node) -> Result<OtherSqlXml, String> {
    let database_type = attr_string_required(node, "databaseType")?;
    let order_text = attr_string_required(node, "order")?;
    let order = match order_text.to_ascii_lowercase().as_str() {
        "top" => OtherSqlOrderXml::Top,
        "bottom" => OtherSqlOrderXml::Bottom,
        other => {
            return Err(format!(
                "<otherSql> element has an unrecognized order '{}' (expected top or bottom)",
                other
            ));
        }
    };
    Ok(OtherSqlXml { database_type, order, sql: collect_text(node) })
}

fn parse_enum_node(node: Node) -> Result<EnumXml, String> {
    let name = attr_string_required(node, "name")?;
    let mut value = Vec::new();
    for v in node.children().filter(|n| n.has_tag_name((NS, "value"))) {
        value.push(EnumValueXml { name: attr_string_required(v, "name")?, code: attr_string(v, "code") });
    }
    Ok(EnumXml { name, value })
}

fn parse_triggers_node(node: Node) -> Result<TriggersXml, String> {
    let mut update = Vec::new();
    let mut delete = Vec::new();
    for c in node.children().filter(|n| n.is_element()) {
        if c.has_tag_name((NS, "update")) {
            update.push(TriggerXml { database_type: attr_string_required(c, "databaseType")?, sql: collect_text(c) });
        } else if c.has_tag_name((NS, "delete")) {
            delete.push(TriggerXml { database_type: attr_string_required(c, "databaseType")?, sql: collect_text(c) });
        }
    }
    Ok(TriggersXml { update, delete })
}

fn parse_constraints_node(node: Node) -> Result<ConstraintsXml, String> {
    let mut constraint = Vec::new();
    for c in node.children().filter(|n| n.has_tag_name((NS, "constraint"))) {
        constraint.push(ConstraintXml {
            name: attr_string_required(c, "name")?,
            database_type: attr_string(c, "databaseType"),
            sql: collect_text(c),
        });
    }
    Ok(ConstraintsXml { constraint })
}

fn parse_aggregations_node(node: Node) -> Result<AggregationsXml, String> {
    let mut aggregate = Vec::new();
    for a in node.children().filter(|n| n.has_tag_name((NS, "aggregate"))) {
        aggregate.push(parse_aggregate_node(a)?);
    }
    Ok(AggregationsXml { aggregate })
}

fn parse_aggregate_node(node: Node) -> Result<AggregateXml, String> {
    let mut sum = Vec::new();
    let mut count = Vec::new();
    let mut group: Option<GroupXml> = None;

    for c in node.children().filter(|n| n.is_element()) {
        if c.has_tag_name((NS, "sum")) {
            sum.push(SumXml {
                source_column: attr_string_required(c, "sourceColumn")?,
                destination_column: attr_string_required(c, "destinationColumn")?,
            });
        } else if c.has_tag_name((NS, "count")) {
            count.push(CountXml { destination_column: attr_string_required(c, "destinationColumn")? });
        } else if c.has_tag_name((NS, "group")) {
            group = Some(parse_group_node(c)?);
        }
    }

    Ok(AggregateXml {
        sum,
        count,
        group: group.unwrap_or_else(|| GroupXml { column: Vec::new() }),
        destination_table: attr_string_required(node, "destinationTable")?,
        date_column: attr_string_required(node, "dateColumn")?,
        timestamp_column: attr_string_required(node, "timestampColumn")?,
        frequency: attr_string_required(node, "frequency")?,
        criteria: attr_string(node, "criteria"),
    })
}

fn parse_group_node(node: Node) -> Result<GroupXml, String> {
    let mut column = Vec::new();
    for c in node.children().filter(|n| n.has_tag_name((NS, "column"))) {
        column.push(GroupColumnXml {
            source: attr_string_required(c, "source")?,
            destination: attr_string_required(c, "destination")?,
            source_derived_from: attr_string(c, "sourceDerivedFrom"),
        });
    }
    Ok(GroupXml { column })
}

fn parse_initial_data_node(node: Node) -> InitialDataXml {
    let mut sql = Vec::new();
    for s in node.children().filter(|n| n.has_tag_name((NS, "sql"))) {
        sql.push(InitialSqlXml {
            database_type: attr_string(s, "databaseType"),
            sql: collect_text(s),
        });
    }
    InitialDataXml { sql }
}

// -------- Utils --------
fn attr_string(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(|s| s.to_string())
}

/// Reads a required attribute. Returns an error (naming the element and attribute)
/// rather than silently defaulting to `""` when it's absent - a missing `name`/`type`/
/// `table`/`column`/etc. attribute is a malformed schema definition, and nothing
/// downstream validates against an empty name, so a silent default would otherwise let
/// it through as a real (if oddly-named) table/column/relation.
fn attr_string_required(node: Node, name: &str) -> Result<String, String> {
    node.attribute(name).map(|s| s.to_string()).ok_or_else(|| {
        format!(
            "<{}> element is missing required attribute '{}'",
            node.tag_name().name(),
            name
        )
    })
}

fn attr_bool(node: Node, name: &str) -> Option<bool> {
    node.attribute(name).and_then(|v| match v.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn attr_i32(node: Node, name: &str) -> Option<i32> {
    node.attribute(name).and_then(|v| v.parse::<i32>().ok())
}

fn attr_f64(node: Node, name: &str) -> Option<f64> {
    node.attribute(name).and_then(|v| v.parse::<f64>().ok())
}

fn collect_text(node: Node) -> String {
    let mut out = String::new();
    for n in node.children() {
        if n.is_text() {
            out.push_str(n.text().unwrap_or(""));
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_with_roxmltree_sample() {
        let xml = fs::read_to_string("tests/resources/schema-parser-test-schema.xml").expect("resource present");
        let db = parse_database_roxml(&xml).expect("parsed");
        assert!(!db.tables.is_empty());
        assert!(db.schemas.is_empty() || !db.schemas[0].tables.is_empty());
    }

    #[test]
    fn attr_bool_is_case_insensitive() {
        let doc = Document::parse(
            r#"<column xmlns="http://stano.com/database" name="x" type="int" required="TRUE" cluster="No"/>"#,
        )
        .unwrap();
        let node = doc.root_element();

        assert_eq!(attr_bool(node, "required"), Some(true));
        assert_eq!(attr_bool(node, "cluster"), Some(false));
    }

    #[test]
    fn parse_column_node_honors_uppercase_required_attribute() {
        let doc = Document::parse(
            r#"<column xmlns="http://stano.com/database" name="x" type="int" required="TRUE"/>"#,
        )
        .unwrap();
        let node = doc.root_element();

        let column = parse_column_node(node).expect("parse ok");

        assert_eq!(column.required, Some(true));
    }

    #[test]
    fn missing_required_attribute_returns_error_instead_of_defaulting_to_empty_string() {
        let doc = Document::parse(r#"<column xmlns="http://stano.com/database" type="int"/>"#).unwrap();
        let node = doc.root_element();

        let err = parse_column_node(node).unwrap_err();
        assert!(err.contains("column"));
        assert!(err.contains("name"));
    }
}
