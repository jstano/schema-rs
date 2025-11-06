use crate::common::generator_context::GeneratorContext;
use crate::common::view_generator::{DefaultViewGenerator, ViewGenerator};

pub struct H2ViewGenerator {
    view_generator: DefaultViewGenerator,
}

impl H2ViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            view_generator: DefaultViewGenerator::new(context.clone()),
        }
    }
}

impl ViewGenerator for H2ViewGenerator {
    fn output_views(&self) {
        self.view_generator.output_views();
    }
}
