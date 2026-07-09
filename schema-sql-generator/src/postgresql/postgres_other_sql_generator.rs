use crate::common::generator_context::GeneratorContext;
use crate::common::other_sql_generator::{DefaultOtherSqlGenerator, OtherSqlGenerator};
use crate::common::sql_writer::SqlWriter;

pub struct PostgresOtherSqlGenerator {
    other_sql_generator: DefaultOtherSqlGenerator
}

impl PostgresOtherSqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            other_sql_generator: DefaultOtherSqlGenerator::new(context),
        }
    }
}

impl OtherSqlGenerator for PostgresOtherSqlGenerator {
    fn output_other_sql_top(&self) {
        self.other_sql_generator.output_other_sql_top();
    }

    fn output_other_sql_bottom(&self) {
        self.other_sql_generator.output_other_sql_bottom();
    }

    fn output_other_sql(&self, writer: &mut SqlWriter, statement_separator: &str, sql: &str) {
        self.other_sql_generator.output_other_sql(writer, statement_separator, sql);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::other_sql::OtherSql;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, OtherSqlOrder};

    #[test]
    fn output_other_sql_top_renders_matching_entries_only() {
        let schema = SchemaBuilder::new(None::<&str>)
            .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Top, "create extension if not exists citext"))
            .add_other_sql(OtherSql::new(DatabaseType::Sqlite, OtherSqlOrder::Top, "pragma foreign_keys = on"))
            .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Bottom, "analyze"))
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresOtherSqlGenerator::new(ctx);
        generator.output_other_sql_top();

        let output = buffer.contents();
        assert!(output.contains("create extension if not exists citext;"));
        assert!(!output.contains("pragma"));
        assert!(!output.contains("analyze"));
    }

    #[test]
    fn output_other_sql_bottom_renders_matching_entries_only() {
        let schema = SchemaBuilder::new(None::<&str>)
            .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Bottom, "analyze"))
            .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Top, "create extension if not exists citext"))
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresOtherSqlGenerator::new(ctx);
        generator.output_other_sql_bottom();

        let output = buffer.contents();
        assert!(output.contains("analyze;"));
        assert!(!output.contains("citext"));
    }
}
