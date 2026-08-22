use crate::common::column_type_label::column_type_label;
use crate::common::diagram_generator::DiagramGenerator;
use crate::common::safe_identifier::{build_safe_identifier_map, sanitize_token};
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::RelationType;
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
        let table_names: Vec<String> = tables.iter().map(|t| t.name().to_uppercase()).collect();
        let table_id_map = build_safe_identifier_map(table_names.iter().map(|s| s.as_str()));

        for (i, (table, raw_table_name)) in tables.iter().zip(table_names.iter()).enumerate() {
            let table_alias = &table_id_map[raw_table_name.as_str()];
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
            let column_id_map = build_safe_identifier_map(column_names.iter().copied());

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
                output.push_str(&format!("  * {} : {} <<PK>>\n", column_id_map[col.name()], type_label));
            }

            if !pk_cols.is_empty() {
                output.push_str("  --\n");
            }

            for col in &non_pk_cols {
                let type_label = column_type_label(col.column_type());
                let col_token = &column_id_map[col.name()];
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
        for (table, raw_table_name) in tables.iter().zip(table_names.iter()) {
            for relation in table.relations() {
                if !has_relations {
                    output.push('\n');
                    has_relations = true;
                }
                let from_table = &table_id_map[raw_table_name.as_str()];
                let raw_to_table = relation.to_table_name().to_uppercase();
                // The common case: an unqualified reference that matches a real table's
                // own name exactly, so it shares that table's (possibly disambiguated)
                // alias. A qualified or otherwise non-matching reference falls back to
                // sanitizing it standalone, same as before.
                let to_table = table_id_map
                    .get(raw_to_table.as_str())
                    .cloned()
                    .unwrap_or_else(|| sanitize_token(&raw_to_table));
                let cardinality = match relation.relation_type() {
                    RelationType::Enforce | RelationType::Cascade => "}o--||",
                    RelationType::SetNull | RelationType::DoNothing => "}o--o|",
                };
                let from_col = sanitize_token(relation.from_column_name());
                output.push_str(&format!(
                    "{} {} {} : {}\n",
                    from_table, cardinality, to_table, from_col
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
}
