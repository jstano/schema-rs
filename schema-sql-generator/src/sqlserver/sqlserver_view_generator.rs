use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::ViewGenerator;
use crate::sql_println;
use schema_model::model::view::View;

pub struct SqlServerViewGenerator {
    context: GeneratorContext,
}

impl SqlServerViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    fn get_schema_name(view: &View) -> String {
        match view.schema_name() {
            Some(schema) if schema.eq_ignore_ascii_case("public") => "dbo".to_string(),
            Some(schema) => schema.to_string(),
            None => "dbo".to_string(),
        }
    }

    fn get_fully_qualified_name(view: &View) -> String {
        format!("{}.{}", Self::get_schema_name(view), view.name())
    }
}

impl ViewGenerator for SqlServerViewGenerator {
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
                    let view_name = Self::get_fully_qualified_name(&view);
                    sql_println!(writer, "/* {} */", view_name);
                    sql_println!(writer, "if exists (select name from dbo.sysobjects where name = '{}' and type = 'V')", view.name());
                    sql_println!(writer, "   drop view {}{}", view_name, separator);
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
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_views_maps_public_schema_to_dbo() {
        let view = View::new(Some("public"), "active_users", "select * from users where active = 1", None);
        let schema = SchemaBuilder::new(Some("public")).add_view(view).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerViewGenerator::new(ctx);
        generator.output_views();

        let output = buffer.contents();
        assert!(output.contains("dbo.active_users"));
        assert!(!output.contains("public.active_users"));
        assert!(output.contains("if exists (select name from dbo.sysobjects where name = 'active_users' and type = 'V')"));
        assert!(output.contains("create view dbo.active_users as"));
    }

    #[test]
    fn output_views_preserves_non_public_schema_name() {
        let view = View::new(Some("app"), "active_users", "select 1", None);
        let schema = SchemaBuilder::new(Some("app")).add_view(view).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerViewGenerator::new(ctx);
        generator.output_views();

        assert!(buffer.contents().contains("create view app.active_users as"));
    }

    #[test]
    fn output_views_does_nothing_when_no_views() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerViewGenerator::new(ctx);
        generator.output_views();

        assert_eq!(buffer.contents(), "");
    }
}
