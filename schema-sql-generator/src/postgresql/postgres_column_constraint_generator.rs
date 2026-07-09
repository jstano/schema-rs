use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;

pub struct PostgresColumnConstraintGenerator {
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl PostgresColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context),
        }
    }
}

impl ColumnConstraintGenerator for PostgresColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        let boolean_mode = self.column_constraint_generator.context().settings().boolean_mode();
        table.columns_with_check_constraints(boolean_mode)
            .iter()
            .filter(|col| col.column_type() != ColumnType::Enum)
            .map(|col| self.column_constraint_generator.generate_constraint(table, col))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn min_max_constraint_is_rendered() {
        let table = TableBuilder::new(None::<&str>, "products")
            .add_column(
                ColumnBuilder::new(None::<&str>, "price", ColumnType::Int)
                    .min_value(Some(0.0))
                    .max_value(Some(100.0))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(price >= 0 and price <= 100)"));
    }

    #[test]
    fn enum_columns_are_excluded_unlike_default_behavior() {
        // Postgres represents enums as native enum types, so unlike the generic default
        // generator, enum columns must never get a "check(... in (...))" constraint here.
        let table = TableBuilder::new(None::<&str>, "accounts")
            .add_column(
                ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
                    .enum_type(Some("status_type".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresColumnConstraintGenerator::new(ctx);
        assert!(generator.column_check_constraints(&table).is_empty());
    }
}
