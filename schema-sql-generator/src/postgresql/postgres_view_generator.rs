use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::ViewGenerator;
use crate::sql_println;
use schema_model::model::view::View;

pub struct PostgresViewGenerator {
    context: GeneratorContext,
}

impl PostgresViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }
}

impl ViewGenerator for PostgresViewGenerator {
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
                    let view_name = view.fully_qualified_view_name();
                    sql_println!(writer, "/* {} */", view_name);
                    sql_println!(writer, "create or replace view {} as", view_name);
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
    use schema_model::model::types::{BooleanMode, ForeignKeyMode};
    use schema_model::model::types::DatabaseType as ModelDatabaseType;

    #[test]
    fn output_views_renders_create_or_replace_view() {
        let view = View::new(Some("app"), "active_users", "select * from users where active = true", None);
        let schema = SchemaBuilder::new(Some("app")).add_view(view).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, ModelDatabaseType::Postgresql);

        let generator = PostgresViewGenerator::new(ctx);
        generator.output_views();

        let output = buffer.contents();
        assert!(output.contains("create or replace view app.active_users as"));
        assert!(output.contains("select * from users where active = true;"));
    }

    #[test]
    fn output_views_does_nothing_when_no_views() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, ModelDatabaseType::Postgresql);

        let generator = PostgresViewGenerator::new(ctx);
        generator.output_views();

        assert_eq!(buffer.contents(), "");
    }
}
