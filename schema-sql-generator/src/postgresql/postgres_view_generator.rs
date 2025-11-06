use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::{DefaultViewGenerator, ViewGenerator};

pub struct PostgresViewGenerator {
    view_generator: DefaultViewGenerator,
}

impl PostgresViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            view_generator: DefaultViewGenerator::new(context),
        }
    }
}

impl ViewGenerator for PostgresViewGenerator {
    fn output_views(&self) {
        self.view_generator.output_views();
    }
}
