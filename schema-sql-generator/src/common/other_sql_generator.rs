use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::types::OtherSqlOrder;

pub trait OtherSqlGenerator {
    fn output_other_sql_top(&self);
    fn output_other_sql_bottom(&self);
    fn output_other_sql(&self, writer: &mut SqlWriter, statement_separator: &str, sql: &str);
}

pub struct DefaultOtherSqlGenerator {
    context: GeneratorContext,
}

impl DefaultOtherSqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl DefaultOtherSqlGenerator {
    /// Same iteration/filtering logic as `output_other_sql_top`, but dispatches each
    /// entry through `generator` rather than `self` - see
    /// `DefaultFunctionGenerator::output_functions_via` for why: a dialect wrapper (e.g.
    /// `PostgresOtherSqlGenerator`) that overrides `output_other_sql` but delegates
    /// `output_other_sql_top` straight to this struct would otherwise never see its own
    /// override invoked, because Rust has no virtual dispatch on concrete types. Callers
    /// that need their override honored should pass `self` (as a `&dyn OtherSqlGenerator`)
    /// here instead of calling `output_other_sql_top` directly.
    pub fn output_other_sql_top_via(&self, generator: &dyn OtherSqlGenerator) {
        let database_type = self.context.settings().database_type();
        let statement_separator = self.context.settings().statement_separator();
        let database_model = self.context.settings().database_model();

        self.context.with_writer(|writer| {
            database_model.schemas().iter().for_each(|schema| {
                schema
                    .other_sql()
                    .iter()
                    .filter(|sql| sql.database_type() == database_type)
                    .filter(|sql| sql.order() == OtherSqlOrder::Top)
                    .filter(|sql| !sql.sql().is_empty())
                    .for_each(|sql| {
                        generator.output_other_sql(writer, statement_separator, sql.sql());
                    })
            });
        });
    }

    /// Same as `output_other_sql_top_via`, for the bottom-ordered entries - see that
    /// method for why dispatching through `generator` (rather than calling
    /// `self.output_other_sql` directly) matters.
    pub fn output_other_sql_bottom_via(&self, generator: &dyn OtherSqlGenerator) {
        let database_type = self.context.settings().database_type();
        let statement_separator = self.context.settings().statement_separator();
        let database_model = self.context.settings().database_model();

        self.context.with_writer(|writer| {
            database_model.schemas().iter().for_each(|schema| {
                schema
                    .other_sql()
                    .iter()
                    .filter(|sql| sql.database_type() == database_type)
                    .filter(|sql| sql.order() == OtherSqlOrder::Bottom)
                    .filter(|sql| !sql.sql().is_empty())
                    .for_each(|sql| {
                        generator.output_other_sql(writer, statement_separator, sql.sql());
                    })
            });
        });
    }
}

impl OtherSqlGenerator for DefaultOtherSqlGenerator {
    fn output_other_sql_top(&self) {
        self.output_other_sql_top_via(self);
    }

    fn output_other_sql_bottom(&self) {
        self.output_other_sql_bottom_via(self);
    }

    fn output_other_sql(&self, writer: &mut SqlWriter, statement_separator: &str, sql: &str) {
        writer.print(sql);
        writer.println(statement_separator);
        writer.newline();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::other_sql::OtherSql;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    struct TaggingOtherSqlGenerator(DefaultOtherSqlGenerator);

    impl OtherSqlGenerator for TaggingOtherSqlGenerator {
        fn output_other_sql_top(&self) {
            self.0.output_other_sql_top_via(self);
        }

        fn output_other_sql_bottom(&self) {
            self.0.output_other_sql_bottom_via(self);
        }

        fn output_other_sql(&self, writer: &mut SqlWriter, statement_separator: &str, sql: &str) {
            writer.print("-- tagged\n");
            self.0.output_other_sql(writer, statement_separator, sql);
        }
    }

    #[test]
    fn output_other_sql_top_via_dispatches_through_the_passed_generator_not_self() {
        // Regression test for M10: a wrapper around `DefaultOtherSqlGenerator` must have
        // its own `output_other_sql` override honored when `output_other_sql_top`/`_bottom`
        // call `output_other_sql_{top,bottom}_via(self)` rather than plainly delegating to
        // `DefaultOtherSqlGenerator`'s own methods - the latter would call back only into
        // `DefaultOtherSqlGenerator::output_other_sql`, silently skipping the wrapper's
        // override.
        let schema = SchemaBuilder::new(None::<&str>)
            .add_other_sql(OtherSql::new(DatabaseType::Postgresql, schema_model::model::types::OtherSqlOrder::Top, "select 1"))
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = TaggingOtherSqlGenerator(DefaultOtherSqlGenerator::new(ctx));
        generator.output_other_sql_top();

        let output = buffer.contents();
        assert!(output.contains("-- tagged"));
        assert!(output.contains("select 1"));
    }
}
