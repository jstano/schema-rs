use crate::common::generator_context::GeneratorContext;

pub trait ViewGenerator {
    fn output_views(&self);
}

pub struct DefaultViewGenerator {
    context: GeneratorContext,
}

impl DefaultViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ViewGenerator for DefaultViewGenerator {
    fn output_views(&self) {
    }
}
