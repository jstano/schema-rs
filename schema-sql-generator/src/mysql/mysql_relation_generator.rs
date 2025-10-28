use crate::common::generator_context::GeneratorContext;
use crate::common::relation_generator::{DefaultRelationGenerator, RelationGenerator};

pub struct MySqlRelationGenerator {
    context: GeneratorContext,
    relation_generator: DefaultRelationGenerator,
}

impl MySqlRelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            relation_generator: DefaultRelationGenerator::new(context.clone()),
            context,
        }
    }
}

impl RelationGenerator for MySqlRelationGenerator {
    fn output_relations(&self) {
        self.relation_generator.output_relations();
    }
}
