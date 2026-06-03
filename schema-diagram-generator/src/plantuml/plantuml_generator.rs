use std::rc::Rc;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::RelationType;
use crate::common::column_type_label::column_type_label;
use crate::common::diagram_generator::DiagramGenerator;

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

        for (i, table) in tables.iter().enumerate() {
            let table_name = table.name().to_uppercase();
            output.push_str(&format!("entity {} {{\n", table_name));

            // Collect PK column names
            let pk_columns: Vec<String> = table
                .primary_key()
                .map(|pk| pk.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();

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
                output.push_str(&format!("  * {} : {} <<PK>>\n", col.name(), type_label));
            }

            if !pk_cols.is_empty() {
                output.push_str("  --\n");
            }

            for col in &non_pk_cols {
                let type_label = column_type_label(col.column_type());
                let col_name = col.name();
                if table.column_relation(col).is_some() {
                    output.push_str(&format!("  {} : {} <<FK>>\n", col_name, type_label));
                } else {
                    output.push_str(&format!("  {} : {}\n", col_name, type_label));
                }
            }

            output.push_str("}\n");

            if i < table_count - 1 {
                output.push('\n');
            }
        }

        // Relations
        let mut has_relations = false;
        for table in self.database_model.all_tables() {
            for relation in table.relations() {
                if !has_relations {
                    output.push('\n');
                    has_relations = true;
                }
                let from_table = table.name().to_uppercase();
                let to_table = relation.to_table_name().to_uppercase();
                let cardinality = match relation.relation_type() {
                    RelationType::Enforce | RelationType::Cascade => "}o--||",
                    RelationType::SetNull | RelationType::DoNothing => "}o--o|",
                };
                let from_col = relation.from_column_name();
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
                RelationType::Cascade,
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
}
