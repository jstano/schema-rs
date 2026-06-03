use std::rc::Rc;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::RelationType;
use crate::common::column_type_label::column_type_label;
use crate::common::diagram_generator::DiagramGenerator;

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

        for table in self.database_model.all_tables() {
            let table_name = table.name().to_uppercase();
            output.push_str(&format!("    {} {{\n", table_name));

            // Collect PK column names
            let pk_columns: Vec<String> = table
                .primary_key()
                .map(|pk| pk.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();

            for col in table.columns() {
                let type_label = column_type_label(col.column_type());
                let col_name = col.name();

                let annotation = if pk_columns.iter().any(|pk| pk.eq_ignore_ascii_case(col_name)) {
                    " PK"
                } else if table.column_relation(col).is_some() {
                    " FK"
                } else {
                    ""
                };

                output.push_str(&format!("        {} {}{}\n", type_label, col_name, annotation));
            }

            output.push_str("    }\n");
        }

        // Relations
        for table in self.database_model.all_tables() {
            let from_table = table.name().to_uppercase();
            for relation in table.relations() {
                let to_table = relation.to_table_name().to_uppercase();
                let cardinality = match relation.relation_type() {
                    RelationType::Enforce | RelationType::Cascade => "}o--||",
                    RelationType::SetNull | RelationType::DoNothing => "}o--o|",
                };
                let from_col = relation.from_column_name();
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
    use std::rc::Rc;
    use schema_model::builder::{ColumnBuilder, KeyBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::relation::Relation;
    use schema_model::model::types::{BooleanMode, ForeignKeyMode, KeyType, RelationType};

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

        DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
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
}
