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
