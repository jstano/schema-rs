use crate::common::generator_context::GeneratorContext;
use crate::common::relation_generator::{DefaultRelationGenerator, RelationGenerator};

pub struct PostgresRelationGenerator {
    context: GeneratorContext,
    relation_generator: DefaultRelationGenerator,
}

impl PostgresRelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            relation_generator: DefaultRelationGenerator::new(context.clone()),
            context,
        }
    }
}

impl RelationGenerator for PostgresRelationGenerator {
    fn output_relations(&self) {
        self.relation_generator.output_relations();
    }
}
