use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::{DefaultViewGenerator, ViewGenerator};

pub struct PostgresViewGenerator {
    context: GeneratorContext,
    view_generator: DefaultViewGenerator,
}

impl PostgresViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            view_generator: DefaultViewGenerator::new(context.clone()),
            context,
        }
    }
}

impl ViewGenerator for PostgresViewGenerator {
    fn output_views(&self) {
        self.view_generator.output_views();
    }
}
