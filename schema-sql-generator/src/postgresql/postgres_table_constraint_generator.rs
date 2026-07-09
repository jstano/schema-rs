use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;
use crate::common::table_constraint_generator::{DefaultTableConstraintGenerator, TableConstraintGenerator};

pub struct PostgresTableConstraintGenerator {
    table_constraint_generator: DefaultTableConstraintGenerator,
}

impl PostgresTableConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_constraint_generator: DefaultTableConstraintGenerator::new(context),
        }
    }   
}

impl TableConstraintGenerator for PostgresTableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String> {
        self.table_constraint_generator.table_check_constraints(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{SchemaBuilder, TableBuilder};
    use schema_model::model::constraint::Constraint;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn table_check_constraints_renders_each_constraint() {
        let table = TableBuilder::new(None::<&str>, "orders")
            .add_constraint(Constraint::new("ck_total_positive", "check (total > 0)", DatabaseType::Postgresql))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresTableConstraintGenerator::new(ctx);
        let constraints = generator.table_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("constraint ck_total_positive check (total > 0)"));
    }

    #[test]
    fn no_constraints_when_table_has_none() {
        let table = TableBuilder::new(None::<&str>, "orders").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresTableConstraintGenerator::new(ctx);
        assert!(generator.table_check_constraints(&table).is_empty());
    }
}
