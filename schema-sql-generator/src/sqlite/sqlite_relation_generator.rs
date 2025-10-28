use crate::common::generator_context::GeneratorContext;
use crate::common::relation_generator::{DefaultRelationGenerator, RelationGenerator};

pub struct SqliteRelationGenerator {
    context: GeneratorContext,
    relation_generator: DefaultRelationGenerator,
}

impl SqliteRelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            relation_generator: DefaultRelationGenerator::new(context.clone()),
            context,
        }
    }
}

impl RelationGenerator for SqliteRelationGenerator {
    fn output_relations(&self) {
        self.relation_generator.output_relations();
    }
}
