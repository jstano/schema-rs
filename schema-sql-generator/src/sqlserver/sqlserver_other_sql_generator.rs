use crate::common::generator_context::GeneratorContext;
use crate::common::other_sql_generator::{DefaultOtherSqlGenerator, OtherSqlGenerator};
use crate::common::sql_writer::SqlWriter;

pub struct SqlServerOtherSqlGenerator {
    other_sql_generator: DefaultOtherSqlGenerator
}

impl SqlServerOtherSqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            other_sql_generator: DefaultOtherSqlGenerator::new(context),
        }
    }
}

impl OtherSqlGenerator for SqlServerOtherSqlGenerator {
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
