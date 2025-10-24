use crate::common::generator_context::GeneratorContext;

pub trait RelationGenerator {
    fn output_relations(&self);
}

pub struct DefaultRelationGenerator {
    context: GeneratorContext,
}

impl DefaultRelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl RelationGenerator for DefaultRelationGenerator {
    fn output_relations(&self) {
    }
}
