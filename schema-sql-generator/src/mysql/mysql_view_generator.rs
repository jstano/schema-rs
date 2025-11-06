use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::{DefaultViewGenerator, ViewGenerator};

pub struct MySqlViewGenerator {
    view_generator: DefaultViewGenerator,
}

impl MySqlViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            view_generator: DefaultViewGenerator::new(context),
        }
    }
}

impl ViewGenerator for MySqlViewGenerator {
    fn output_views(&self) {
        self.view_generator.output_views();
    }
}
