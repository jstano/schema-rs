use schema_model::model::table::Table;
use crate::common::column_constraint_generator::{ColumnConstraintGenerator, DefaultColumnConstraintGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqlServerColumnConstraintGenerator {
    column_constraint_generator: DefaultColumnConstraintGenerator,
}

impl SqlServerColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_constraint_generator: DefaultColumnConstraintGenerator::new(context),
        }
    }
}

impl ColumnConstraintGenerator for SqlServerColumnConstraintGenerator {
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
    use schema_model::model::enum_type::{EnumType, EnumValue};
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
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(price >= 0 and price <= 100)"));
    }

    #[test]
    fn enum_columns_are_included_unlike_postgres() {
        // Unlike PostgresColumnConstraintGenerator (which excludes enum columns because it
        // uses native postgres enum types), SQL Server has no native enum type, so it must
        // keep the generic "check(... in (...))" constraint for enum columns.
        let enum_type = EnumType::new("status_type", vec![EnumValue::new("ACTIVE", Some("A".to_string()))]);
        let table = TableBuilder::new(None::<&str>, "accounts")
            .add_column(
                ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
                    .enum_type(Some("status_type".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .add_enum_type(enum_type)
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(status in ('A'))"));
    }
}
