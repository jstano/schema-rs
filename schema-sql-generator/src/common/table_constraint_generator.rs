use crate::common::generator_context::GeneratorContext;
use schema_model::model::constraint::Constraint;
use schema_model::model::table::Table;
#[cfg(test)]
use schema_model::model::types::DatabaseType;

pub trait TableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultTableConstraintGenerator {
    context: GeneratorContext,
}

impl DefaultTableConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn generator_constraint(&self, constraint: &Constraint) -> String {
        format!("   constraint {} {}",
               constraint.name(),
               constraint.sql())
    }
}

impl TableConstraintGenerator for DefaultTableConstraintGenerator {
    fn table_check_constraints(&self, table: &Table) -> Vec<String> {
        let database_type = self.context.settings().database_type();
        table.constraints().iter()
            .filter(|constraint| constraint.database_type() == database_type)
            .map(|constraint| {
                self.generator_constraint(constraint)
            }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, ForeignKeyMode};

    /// H18: a table-level `<constraint databaseType="…">` must only be emitted into the
    /// script for the database it targets - the same rule already applied to views
    /// (`Schema::views`) and initial data (`output_initial_data`).
    #[test]
    fn table_check_constraints_filters_by_database_type() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_constraint(Constraint::new("ck_email_pg", "check (email ~ '^[^@]+@')", DatabaseType::Postgresql))
            .add_constraint(Constraint::new("ck_email_lite", "check (email like '%@%')", DatabaseType::Sqlite))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = DefaultTableConstraintGenerator::new(ctx);
        let constraints = generator.table_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("ck_email_lite"));
        assert!(!constraints[0].contains("ck_email_pg"));
    }
}
