use crate::common::generator_context::GeneratorContext;
use crate::common::other_sql_generator::{DefaultOtherSqlGenerator, OtherSqlGenerator};

pub struct H2OtherSqlGenerator {
    context: GeneratorContext,
    other_sql_generator: DefaultOtherSqlGenerator
}

impl H2OtherSqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            other_sql_generator: DefaultOtherSqlGenerator::new(context.clone()),
            context,
        }
    }
}

impl OtherSqlGenerator for H2OtherSqlGenerator {
    fn output_other_sql_top(&self) {
        self.other_sql_generator.output_other_sql_top();
    }

    fn output_other_sql_bottom(&self) {
        self.other_sql_generator.output_other_sql_bottom();
    }
}
