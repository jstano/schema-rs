use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::{DefaultViewGenerator, ViewGenerator};

pub struct H2PostgresViewGenerator {
    context: GeneratorContext,
    view_generator: DefaultViewGenerator,
}

impl H2PostgresViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            view_generator: DefaultViewGenerator::new(context.clone()),
            context,
        }
    }
}

impl ViewGenerator for H2PostgresViewGenerator {
    fn output_views(&self) {
        self.view_generator.output_views();
    }
}
