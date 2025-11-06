use crate::common::generator_context::GeneratorContext;
use crate::common::trigger_generator::{DefaultTriggerGenerator, TriggerGenerator};

pub struct SqlServerTriggerGenerator {
    trigger_generator: DefaultTriggerGenerator,
}

impl SqlServerTriggerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            trigger_generator: DefaultTriggerGenerator::new(context),
        }
    }
}

impl TriggerGenerator for SqlServerTriggerGenerator {
    fn output_triggers(&self) {
        self.trigger_generator.output_triggers();
    }
}
