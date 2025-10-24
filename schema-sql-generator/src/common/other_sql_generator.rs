use crate::common::generator_context::GeneratorContext;

pub trait OtherSqlGenerator {
    fn output_other_sql_top(&self);
    fn output_other_sql_bottom(&self);
}

pub struct DefaultOtherSqlGenerator {
    context: GeneratorContext,
}

impl DefaultOtherSqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl OtherSqlGenerator for DefaultOtherSqlGenerator {
    fn output_other_sql_top(&self) {
    }

    fn output_other_sql_bottom(&self) {
    }
}
