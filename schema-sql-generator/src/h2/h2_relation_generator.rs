use crate::common::generator_context::GeneratorContext;
use crate::common::relation_generator::{DefaultRelationGenerator, RelationGenerator};

pub struct H2RelationGenerator {
    relation_generator: DefaultRelationGenerator,
}

impl H2RelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            relation_generator: DefaultRelationGenerator::new(context.clone()),
        }
    }
}

impl RelationGenerator for H2RelationGenerator {
    fn output_relations(&self) {
        self.relation_generator.output_relations();
    }
}
