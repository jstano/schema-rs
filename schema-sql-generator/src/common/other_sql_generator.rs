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

impl OtherSqlGenerator for DefaultOtherSqlGenerator {
    fn output_other_sql_top(&self) {
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
                    .filter(|sql| sql.sql().is_empty())
                    .for_each(|sql| {
                        self.output_other_sql(writer, statement_separator, sql.sql());
                    })
            });
        });
    }

    fn output_other_sql_bottom(&self) {
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
                    .filter(|sql| sql.sql().is_empty())
                    .for_each(|sql| {
                        self.output_other_sql(writer, statement_separator, sql.sql());
                    })
            });
        });
    }

    fn output_other_sql(&self, writer: &mut SqlWriter, statement_separator: &str, sql: &str) {
        writer.print(sql);
        writer.println(statement_separator);
        writer.newline();
    }
}
