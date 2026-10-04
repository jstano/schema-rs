use crate::common::column_type_label::column_type_label;
use crate::common::diagram_generator::DiagramGenerator;
use crate::common::relation_support::{relation_cardinality, resolve_relation_target, table_display_name};
use crate::common::safe_identifier::{build_safe_identifiers, sanitize_token};
use schema_model::model::database_model::DatabaseModel;
use std::rc::Rc;

/// Escapes a value for use inside a PlantUML double-quoted string, via PlantUML's
/// standard backslash-escape for embedded quotes.
fn plantuml_escape_quoted(value: &str) -> String {
    value.replace('"', "\\\"")
}

pub struct PlantUMLERDiagramGenerator {
    database_model: Rc<DatabaseModel>,
}

impl PlantUMLERDiagramGenerator {
    pub fn new(database_model: Rc<DatabaseModel>) -> Self {
        Self { database_model }
    }
}

impl DiagramGenerator for PlantUMLERDiagramGenerator {
    fn generate(&self) -> String {
        let mut output = String::new();
        output.push_str("@startuml\n");

        let tables = self.database_model.all_tables();
        let table_count = tables.len();
        // Built once, up front, and reused for every entity block *and* every relation
        // line below - two distinct table names that sanitize to the same token (e.g.
        // "Order-Detail" and "Order Detail" both -> "Order_Detail") must still end up as
        // two distinct, consistently-referenced entity aliases, not silently merged
        // into one.
        let table_names: Vec<String> = tables.iter().map(|t| table_display_name(t)).collect();
        let table_ids = build_safe_identifiers(table_names.iter().map(|s| s.as_str()));

        for (i, (table, raw_table_name)) in tables.iter().zip(table_names.iter()).enumerate() {
            let table_alias = &table_ids[i];
            if table_alias == raw_table_name {
                output.push_str(&format!("entity {} {{\n", table_alias));
            } else {
                // The name needed sanitizing (and/or disambiguating) to stay a safe,
                // unique token; keep it legible via PlantUML's
                // `entity "display name" as Alias` syntax rather than silently losing it.
                output.push_str(&format!(
                    "entity \"{}\" as {} {{\n",
                    plantuml_escape_quoted(raw_table_name),
                    table_alias
                ));
            }

            // Collect PK column names
            let pk_columns: Vec<String> = table
                .primary_key()
                .map(|pk| pk.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();

            let column_names: Vec<&str> = table.columns().iter().map(|c| c.name()).collect();
            let column_ids = build_safe_identifiers(column_names.iter().copied());

            let column_id = |col: &&schema_model::model::column::Column| -> &String {
                let position = table.columns().iter().position(|c| std::ptr::eq(c, *col)).unwrap();
                &column_ids[position]
            };

            // PK columns first
            let pk_cols: Vec<_> = table
                .columns()
                .iter()
                .filter(|col| pk_columns.iter().any(|pk| pk.eq_ignore_ascii_case(col.name())))
                .collect();

            let non_pk_cols: Vec<_> = table
                .columns()
                .iter()
                .filter(|col| !pk_columns.iter().any(|pk| pk.eq_ignore_ascii_case(col.name())))
                .collect();

            for col in &pk_cols {
                let type_label = column_type_label(col.column_type());
                output.push_str(&format!("  * {} : {} <<PK>>\n", column_id(col), type_label));
            }

            if !pk_cols.is_empty() {
                output.push_str("  --\n");
            }

            for col in &non_pk_cols {
                let type_label = column_type_label(col.column_type());
                let col_token = column_id(col);
                if table.column_relation(col).is_some() {
                    output.push_str(&format!("  {} : {} <<FK>>\n", col_token, type_label));
                } else {
                    output.push_str(&format!("  {} : {}\n", col_token, type_label));
                }
            }

            output.push_str("}\n");

            if i < table_count - 1 {
                output.push('\n');
            }
        }

        // Relations
        let mut has_relations = false;
        for (index, table) in tables.iter().enumerate() {
            for relation in table.relations() {
                if !has_relations {
                    output.push('\n');
                    has_relations = true;
                }
                let from_table = &table_ids[index];
                // Resolve the reference to the actual table (honouring `schema.table`) so
                // it shares that table's alias; only an unresolvable reference falls
                // back to sanitizing the raw text.
                let to_table = match resolve_relation_target(&tables, relation.to_table_name()) {
                    Some(i) => table_ids[i].clone(),
                    None => sanitize_token(&relation.to_table_name().to_uppercase()),
                };
                let cardinality = relation_cardinality(table, relation);
                let from_cols = relation
                    .column_pairs()
                    .iter()
                    .map(|(from, _)| sanitize_token(from))
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!(
                    "{} {} {} : {}\n",
                    from_table, cardinality, to_table, from_cols
                ));
            }
        }

        output.push_str("@enduml\n");
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema_model::builder::{ColumnBuilder, KeyBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::relation::Relation;
    use schema_model::model::types::{BooleanMode, ForeignKeyMode, KeyType, RelationType};
    use std::rc::Rc;

    fn build_test_model() -> DatabaseModel {
        let customer_table = TableBuilder::new(None::<&str>, "customer")
            .add_column(
                ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence)
                    .required(true)
                    .build(),
            )
            .add_column(
                ColumnBuilder::new(None::<&str>, "name", ColumnType::Varchar)
                    .length(100)
                    .build(),
            )
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();

        let order_table = TableBuilder::new(None::<&str>, "order")
            .add_column(
                ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence)
                    .required(true)
                    .build(),
            )
            .add_column(
                ColumnBuilder::new(None::<&str>, "customer_id", ColumnType::Int)
                    .required(true)
                    .build(),
            )
            .add_column(
                ColumnBuilder::new(None::<&str>, "created_at", ColumnType::Date)
                    .build(),
            )
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .add_relation(Relation::new(
                "customer",
                "id",
                "order",
                "customer_id",
                RelationType::Cascade,
                false,
            ))
            .build();

        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(customer_table)
            .add_table(order_table)
            .build();

        DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
    }

    #[test]
    fn generates_startuml_and_enduml() {
        let model = Rc::new(build_test_model());
        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.starts_with("@startuml\n"));
        assert!(output.ends_with("@enduml\n"));
    }

    #[test]
    fn generates_entity_blocks() {
        let model = Rc::new(build_test_model());
        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("entity CUSTOMER {"));
        assert!(output.contains("entity ORDER {"));
    }

    #[test]
    fn pk_columns_first_with_separator() {
        let model = Rc::new(build_test_model());
        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("  * id : int <<PK>>"));
        assert!(output.contains("  --"));
    }

    #[test]
    fn fk_columns_annotated() {
        let model = Rc::new(build_test_model());
        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("  customer_id : int <<FK>>"));
    }

    #[test]
    fn relation_line_generated() {
        let model = Rc::new(build_test_model());
        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("ORDER }o--|| CUSTOMER : customer_id"));
    }

    #[test]
    fn composite_relation_lists_all_from_columns() {
        let parent_table = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "tenant_id", ColumnType::Int).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").add_column("tenant_id").build())
            .build();

        let child_table = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "tenant_id", ColumnType::Int).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .add_relation(
                Relation::new_composite(
                    "parent",
                    "child",
                    vec![("parent_id", "id"), ("tenant_id", "tenant_id")],
                    RelationType::Cascade,
                    false,
                )
                .unwrap(),
            )
            .build();

        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent_table)
            .add_table(child_table)
            .build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();

        assert!(output.contains("CHILD }o--|| PARENT : parent_id, tenant_id"), "{output}");
    }

    #[test]
    fn table_name_with_space_is_sanitized_and_kept_legible_via_alias() {
        let table = TableBuilder::new(None::<&str>, "order detail")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();

        assert!(output.contains("entity \"ORDER DETAIL\" as ORDER_DETAIL {"));
        assert!(!output.contains("entity ORDER DETAIL {"));
    }

    #[test]
    fn two_distinct_table_names_that_sanitize_to_the_same_token_get_distinct_aliases() {
        // Regression test: sanitizing each name in isolation would map both
        // "Order-Detail" and "Order Detail" to the alias ORDER_DETAIL, silently
        // merging two distinct tables into one diagram entity.
        let table1 = TableBuilder::new(None::<&str>, "Order-Detail")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let table2 = TableBuilder::new(None::<&str>, "Order Detail")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table1).add_table(table2).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();

        assert_eq!(output.matches("entity ").count(), 2, "expected two distinct entity blocks: {output}");
        assert!(output.contains("as ORDER_DETAIL "));
        assert!(output.contains("as ORDER_DETAIL_2 "));
    }

    #[test]
    fn column_name_with_space_is_sanitized_to_a_single_token() {
        let table = TableBuilder::new(None::<&str>, "widget")
            .add_column(ColumnBuilder::new(None::<&str>, "display name", ColumnType::Varchar).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = PlantUMLERDiagramGenerator::new(model);
        let output = generator.generate();

        assert!(output.contains("  display_name : varchar"));
    }

    fn schema_table(schema: &str, name: &str, relation_to: Option<(&str, bool, RelationType)>) -> schema_model::model::table::Table {
        let mut builder = TableBuilder::new(Some(schema), name)
            .add_column(ColumnBuilder::new(Some(schema), "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(Some(schema), "ref_id", ColumnType::Int).required(relation_to.as_ref().is_some_and(|r| r.1)).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build());
        if let Some((target, _, relation_type)) = relation_to {
            builder = builder.add_relation(Relation::new(target, "id", name, "ref_id", relation_type, false));
        }
        builder.build()
    }

    fn two_schema_model(relation_type: RelationType, required: bool) -> Rc<DatabaseModel> {
        let sales = SchemaBuilder::new(Some("sales"))
            .add_table(schema_table("sales", "users", None))
            .add_table(schema_table("sales", "orders", Some(("auth.users", required, relation_type))))
            .build();
        let auth = SchemaBuilder::new(Some("auth")).add_table(schema_table("auth", "users", None)).build();
        Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![sales, auth]))
    }

    #[test]
    fn schema_qualified_relation_target_matches_a_declared_entity() {
        // M30: the relation used to reference a bare sanitized `AUTH_USERS` that was
        // never declared as an entity.
        let output = PlantUMLERDiagramGenerator::new(two_schema_model(RelationType::Enforce, true)).generate();
        let relation_line = output.lines().find(|l| l.contains("}o--")).expect("relation line");
        let target = relation_line.split_whitespace().nth(2).unwrap();
        assert!(output.contains("AUTH.USERS") && output.contains(target), "{output}");
        assert!(output.lines().any(|l| l.contains("{") && l.contains(target) && !l.contains("}o--")), "target {target} not declared:\n{output}");
    }

    #[test]
    fn same_named_tables_in_different_schemas_get_distinct_entities() {
        // M31: `sales.users` and `auth.users` both resolved to the same id.
        let output = PlantUMLERDiagramGenerator::new(two_schema_model(RelationType::Enforce, true)).generate();
        assert!(output.contains("SALES.USERS") && output.contains("AUTH.USERS"), "{output}");
        let relation_line = output.lines().find(|l| l.contains("}o--")).unwrap();
        assert!(relation_line.starts_with("SALES_ORDERS "), "{relation_line}");
        assert!(relation_line.contains(" AUTH_USERS "), "{relation_line}");
    }

    #[test]
    fn cardinality_follows_nullability_not_delete_action() {
        // M32: a required FK with SET NULL / DO NOTHING is still exactly-one, and an
        // optional FK with CASCADE / ENFORCE is still zero-or-one.
        for (relation_type, required, expected) in [
            (RelationType::DoNothing, true, "}o--||"),
            (RelationType::SetNull, true, "}o--||"),
            (RelationType::Cascade, false, "}o--o|"),
            (RelationType::Enforce, false, "}o--o|"),
        ] {
            let output = PlantUMLERDiagramGenerator::new(two_schema_model(relation_type, required)).generate();
            assert!(output.contains(expected), "{relation_type:?} required={required}:\n{output}");
        }
    }
}
