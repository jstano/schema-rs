use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::{DefaultViewGenerator, ViewGenerator};

pub struct SqlServerViewGenerator {
    view_generator: DefaultViewGenerator,
}

impl SqlServerViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            view_generator: DefaultViewGenerator::new(context),
        }
    }
}

impl ViewGenerator for SqlServerViewGenerator {
    fn output_views(&self) {
        self.view_generator.output_views();
    }
}
