use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::ViewGenerator;
use crate::sql_println;
use schema_model::model::view::View;

pub struct SqliteViewGenerator {
    context: GeneratorContext,
}

impl SqliteViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }
}

impl ViewGenerator for SqliteViewGenerator {
    fn output_views(&self) {
        let database_model = self.context.settings().database_model();
        let database_type = self.context.settings().database_type();
        let separator = self.context.settings().statement_separator();

        let views: Vec<View> = database_model
            .schemas()
            .iter()
            .flat_map(|schema| schema.views(database_type))
            .collect();

        if !views.is_empty() {
            self.context.with_writer(|writer| {
                for view in views {
                    let view_name = view.fully_qualified_view_name(database_type);
                    sql_println!(writer, "/* {} */", view_name);
                    // Unlike Postgres's `create or replace view` and SQL Server's
                    // explicit existence-check guard, SQLite has no "replace" form for
                    // views - `drop view if exists` first keeps re-running the
                    // generated script idempotent (and, unlike `create view if not
                    // exists`, still picks up a changed view definition).
                    sql_println!(writer, "drop view if exists {}{}", view_name, separator);
                    sql_println!(writer, "create view {} as", view_name);
                    sql_println!(writer, "   {}{}", view.sql(), separator);
                    sql_println!(writer, "");
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::DatabaseType as ModelDatabaseType;
    use schema_model::model::types::{BooleanMode, ForeignKeyMode};

    #[test]
    fn output_views_renders_create_view() {
        let view = View::new(None::<&str>, "active_users", "select * from users where active = 1", None);
        let schema = SchemaBuilder::new(None::<&str>).add_view(view).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, ModelDatabaseType::Sqlite);

        let generator = SqliteViewGenerator::new(ctx);
        generator.output_views();

        let output = buffer.contents();
        assert!(output.contains("drop view if exists active_users;"));
        assert!(output.contains("create view active_users as"));
        assert!(output.contains("select * from users where active = 1;"));
    }

    #[test]
    fn output_views_preserves_explicit_schema_name() {
        let view = View::new(Some("app"), "active_users", "select 1", None);
        let schema = SchemaBuilder::new(Some("app")).add_view(view).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, ModelDatabaseType::Sqlite);

        let generator = SqliteViewGenerator::new(ctx);
        generator.output_views();

        assert!(buffer.contents().contains("create view app.active_users as"));
    }

    #[test]
    fn output_views_are_idempotent_when_rerun() {
        // Regression test: re-running the generated SQL against a database that already
        // has the view must not fail with "view already exists".
        let view = View::new(None::<&str>, "active_users", "select 1", None);
        let schema = SchemaBuilder::new(None::<&str>).add_view(view).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, ModelDatabaseType::Sqlite);

        let generator = SqliteViewGenerator::new(ctx);
        generator.output_views();

        let output = buffer.contents();
        let drop_pos = output.find("drop view if exists active_users").unwrap();
        let create_pos = output.find("create view active_users as").unwrap();
        assert!(drop_pos < create_pos, "the drop guard must come before the create statement");
    }

    #[test]
    fn output_views_does_nothing_when_no_views() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, ModelDatabaseType::Sqlite);

        let generator = SqliteViewGenerator::new(ctx);
        generator.output_views();

        assert_eq!(buffer.contents(), "");
    }
}
