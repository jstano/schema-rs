use crate::common::generator_context::GeneratorContext;
use crate::common::trigger_generator::{DefaultTriggerGenerator, TriggerGenerator};

pub struct H2TriggerGenerator {
    context: GeneratorContext,
    trigger_generator: DefaultTriggerGenerator,
}

impl H2TriggerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            trigger_generator: DefaultTriggerGenerator::new(context.clone()),
            context,
        }
    }
}

impl TriggerGenerator for H2TriggerGenerator {
    fn output_triggers(&self) {
        self.trigger_generator.output_triggers();
    }
}
