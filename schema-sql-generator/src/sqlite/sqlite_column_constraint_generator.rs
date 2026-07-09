use schema_model::model::table::Table;
use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqliteColumnConstraintGenerator {
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl SqliteColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context),
        }
    }
}

impl ColumnConstraintGenerator for SqliteColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        self.column_constraint_generator.column_check_constraints(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
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
        let (ctx, _buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(price >= 0 and price <= 100)"));
    }

    #[test]
    fn no_constraints_when_column_has_no_checks() {
        let table = TableBuilder::new(None::<&str>, "products")
            .add_column(ColumnBuilder::new(None::<&str>, "name", ColumnType::Varchar).length(10).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert!(constraints.is_empty());
    }
}
