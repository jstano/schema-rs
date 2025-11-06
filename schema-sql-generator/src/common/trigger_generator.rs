use crate::common::generator_context::GeneratorContext;

pub trait TriggerGenerator {
    fn output_triggers(&self);
}

pub struct DefaultTriggerGenerator {
    context: GeneratorContext,
}

impl DefaultTriggerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl TriggerGenerator for DefaultTriggerGenerator {
    fn output_triggers(&self) {
    }
}