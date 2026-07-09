use crate::common::generator_context::GeneratorContext;
use crate::common::relation_generator::{DefaultRelationGenerator, RelationGenerator};

pub struct SqlServerRelationGenerator {
    relation_generator: DefaultRelationGenerator,
}

impl SqlServerRelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            relation_generator: DefaultRelationGenerator::new(context),
        }
    }
}

impl RelationGenerator for SqlServerRelationGenerator {
    fn output_relations(&self) {
        self.relation_generator.output_relations();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::relation::Relation;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, RelationType};

    #[test]
    fn output_relations_renders_foreign_key_constraint() {
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::DoNothing, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerRelationGenerator::new(ctx);
        generator.output_relations();

        let output = buffer.contents();
        assert!(output.contains("alter table child"));
        assert!(output.contains("foreign key (parent_id) references parent(id)"));
        assert!(output.contains("on delete no action"));
    }

    #[test]
    fn output_relations_does_nothing_when_no_relations_exist() {
        let table = TableBuilder::new(None::<&str>, "solo").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerRelationGenerator::new(ctx);
        generator.output_relations();

        assert_eq!(buffer.contents(), "");
    }
}
