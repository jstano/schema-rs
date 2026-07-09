use crate::common::generator_context::GeneratorContext;
use crate::common::other_sql_generator::{DefaultOtherSqlGenerator, OtherSqlGenerator};
use crate::common::sql_writer::SqlWriter;

pub struct SqliteOtherSqlGenerator {
    other_sql_generator: DefaultOtherSqlGenerator
}

impl SqliteOtherSqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            other_sql_generator: DefaultOtherSqlGenerator::new(context),
        }
    }
}

impl OtherSqlGenerator for SqliteOtherSqlGenerator {
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
            .add_other_sql(OtherSql::new(DatabaseType::Sqlite, OtherSqlOrder::Top, "pragma foreign_keys = on"))
            .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Top, "select 1"))
            .add_other_sql(OtherSql::new(DatabaseType::Sqlite, OtherSqlOrder::Bottom, "vacuum"))
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteOtherSqlGenerator::new(ctx);
        generator.output_other_sql_top();

        let output = buffer.contents();
        assert!(output.contains("pragma foreign_keys = on;"));
        assert!(!output.contains("select 1"));
        assert!(!output.contains("vacuum"));
    }

    #[test]
    fn output_other_sql_bottom_renders_matching_entries_only() {
        let schema = SchemaBuilder::new(None::<&str>)
            .add_other_sql(OtherSql::new(DatabaseType::Sqlite, OtherSqlOrder::Bottom, "vacuum"))
            .add_other_sql(OtherSql::new(DatabaseType::Sqlite, OtherSqlOrder::Top, "pragma foreign_keys = on"))
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteOtherSqlGenerator::new(ctx);
        generator.output_other_sql_bottom();

        let output = buffer.contents();
        assert!(output.contains("vacuum;"));
        assert!(!output.contains("pragma"));
    }
}
