use crate::common::column_type_label::column_type_label;
use crate::common::diagram_generator::DiagramGenerator;
use crate::common::safe_identifier::{build_safe_identifier_map, sanitize_token};
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::RelationType;
use std::rc::Rc;

/// Escapes a value for use inside a Mermaid double-quoted label. Mermaid has no
/// backslash-escape syntax for its diagram text; embedded quotes are escaped with its
/// documented `#quot;` HTML-entity-style code instead.
fn mermaid_escape_label(value: &str) -> String {
    value.replace('"', "#quot;")
}

pub struct MermaidERDiagramGenerator {
    database_model: Rc<DatabaseModel>,
}

impl MermaidERDiagramGenerator {
    pub fn new(database_model: Rc<DatabaseModel>) -> Self {
        Self { database_model }
    }
}

impl DiagramGenerator for MermaidERDiagramGenerator {
    fn generate(&self) -> String {
        let mut output = String::new();
        output.push_str("erDiagram\n");

        let tables = self.database_model.all_tables();
        // Built once, up front, and reused for every entity block *and* every relation
        // line below - two distinct table names that sanitize to the same token (e.g.
        // "Order-Detail" and "Order Detail" both -> "Order_Detail") must still end up as
        // two distinct, consistently-referenced entity ids, not silently merged into one.
        let table_names: Vec<String> = tables.iter().map(|t| t.name().to_uppercase()).collect();
        let table_id_map = build_safe_identifier_map(table_names.iter().map(|s| s.as_str()));

        for (table, raw_table_name) in tables.iter().zip(table_names.iter()) {
            let table_id = &table_id_map[raw_table_name.as_str()];
            if table_id == raw_table_name {
                output.push_str(&format!("    {} {{\n", table_id));
            } else {
                // The name needed sanitizing (and/or disambiguating) to stay a safe,
                // unique token; keep it legible via Mermaid's `id["alias"]` syntax
                // rather than silently losing it.
                output.push_str(&format!(
                    "    {}[\"{}\"] {{\n",
                    table_id,
                    mermaid_escape_label(raw_table_name)
                ));
            }

            // Collect PK column names
            let pk_columns: Vec<String> = table
                .primary_key()
                .map(|pk| pk.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();

            let column_names: Vec<&str> = table.columns().iter().map(|c| c.name()).collect();
            let column_id_map = build_safe_identifier_map(column_names.iter().copied());

            for col in table.columns() {
                let type_label = column_type_label(col.column_type());
                let col_name = col.name();
                let col_token = &column_id_map[col_name];

                let annotation = if pk_columns.iter().any(|pk| pk.eq_ignore_ascii_case(col_name)) {
                    " PK"
                } else if table.column_relation(col).is_some() {
                    " FK"
                } else {
                    ""
                };

                output.push_str(&format!("        {} {}{}\n", type_label, col_token, annotation));
            }

            output.push_str("    }\n");
        }

        // Relations
        for (table, raw_table_name) in tables.iter().zip(table_names.iter()) {
            let from_table = &table_id_map[raw_table_name.as_str()];
            for relation in table.relations() {
                let raw_to_table = relation.to_table_name().to_uppercase();
                // The common case: an unqualified reference that matches a real table's
                // own name exactly, so it shares that table's (possibly disambiguated)
                // entity id. A qualified or otherwise non-matching reference falls back
                // to sanitizing it standalone, same as before.
                let to_table = table_id_map
                    .get(raw_to_table.as_str())
                    .cloned()
                    .unwrap_or_else(|| sanitize_token(&raw_to_table));
                let cardinality = match relation.relation_type() {
                    RelationType::Enforce | RelationType::Cascade => "}o--||",
                    RelationType::SetNull | RelationType::DoNothing => "}o--o|",
                };
                let from_col = mermaid_escape_label(relation.from_column_name());
                output.push_str(&format!(
                    "    {} {} {} : \"{}\"\n",
                    from_table, cardinality, to_table, from_col
                ));
            }
        }

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
                RelationType::Enforce,
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
    fn generates_er_diagram_header() {
        let model = Rc::new(build_test_model());
        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.starts_with("erDiagram\n"));
    }

    #[test]
    fn generates_table_blocks() {
        let model = Rc::new(build_test_model());
        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("    CUSTOMER {"));
        assert!(output.contains("    ORDER {"));
    }

    #[test]
    fn pk_columns_annotated() {
        let model = Rc::new(build_test_model());
        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("int id PK"));
    }

    #[test]
    fn fk_columns_annotated() {
        let model = Rc::new(build_test_model());
        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("int customer_id FK"));
    }

    #[test]
    fn relation_line_generated() {
        let model = Rc::new(build_test_model());
        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();
        assert!(output.contains("ORDER }o--|| CUSTOMER : \"customer_id\""));
    }

    #[test]
    fn table_name_with_space_is_sanitized_and_kept_legible_via_alias() {
        let table = TableBuilder::new(None::<&str>, "order detail")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();

        // The entity id must be a single whitespace-free token...
        assert!(output.contains("ORDER_DETAIL[\"ORDER DETAIL\"] {"));
        // ...and must never contain a raw, structure-breaking space.
        assert!(!output.contains("ORDER DETAIL {"));
    }

    #[test]
    fn two_distinct_table_names_that_sanitize_to_the_same_token_get_distinct_entity_ids() {
        // Regression test: sanitizing each name in isolation would map both
        // "Order-Detail" and "Order Detail" to the entity id ORDER_DETAIL, silently
        // merging two distinct tables into one diagram entity.
        let table1 = TableBuilder::new(None::<&str>, "Order-Detail")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let table2 = TableBuilder::new(None::<&str>, "Order Detail")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table1).add_table(table2).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();

        assert_eq!(output.matches(" {\n").count(), 2, "expected two distinct entity blocks: {output}");
        assert!(output.contains("ORDER_DETAIL {") || output.contains("ORDER_DETAIL[\""));
        assert!(output.contains("ORDER_DETAIL_2["));
    }

    #[test]
    fn column_name_with_space_is_sanitized_to_a_single_token() {
        let table = TableBuilder::new(None::<&str>, "widget")
            .add_column(ColumnBuilder::new(None::<&str>, "display name", ColumnType::Varchar).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();

        assert!(output.contains("varchar display_name"));
    }

    #[test]
    fn relation_label_escapes_embedded_quote() {
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "id\"quoted", ColumnType::Int).build())
            .add_relation(Relation::new("parent", "id", "child", "id\"quoted", RelationType::Enforce, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(parent).add_table(child).build();
        let model = Rc::new(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]));

        let generator = MermaidERDiagramGenerator::new(model);
        let output = generator.generate();

        assert!(output.contains("\"id#quot;quoted\""));
    }
}
