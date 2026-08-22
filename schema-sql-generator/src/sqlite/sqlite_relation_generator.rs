use crate::common::generator_context::GeneratorContext;
use crate::common::relation_generator::{DefaultRelationGenerator, RelationGenerator};
use schema_model::model::table::Table;
use schema_model::model::types::ForeignKeyMode;

pub struct SqliteRelationGenerator {
    relation_generator: DefaultRelationGenerator,
}

impl SqliteRelationGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            relation_generator: DefaultRelationGenerator::new(context),
        }
    }

    /// SQLite has no `ALTER TABLE ... ADD CONSTRAINT` support, so foreign keys can't be
    /// added after the table is created (unlike Postgres/SQL Server). Instead this returns
    /// the `constraint ... foreign key (...) references ...` clauses meant to be embedded
    /// directly inside the table's `CREATE TABLE (...)` definition.
    pub fn inline_foreign_key_constraints(&self, table: &Table) -> Vec<String> {
        let context = self.relation_generator.context();

        if context.settings().foreign_key_mode() != ForeignKeyMode::Relations {
            return Vec::new();
        }

        let database_type = context.settings().database_type();
        let database_model = context.settings().database_model();

        table
            .relations()
            .iter()
            .enumerate()
            .map(|(relation_index, relation)| {
                let constraint_name = self.relation_generator.relation_constraint_name(table, relation_index);
                let to_table = database_model.find_table_by_qualified_name(relation.to_table_name());
                let operation = self.relation_generator.relation_operation_type(relation.relation_type());

                format!(
                    "   constraint {} foreign key ({}) references {}({}) on delete {}",
                    constraint_name,
                    relation.from_column_name(),
                    to_table.fully_qualified_table_name(database_type),
                    relation.to_column_name(),
                    operation
                )
            })
            .collect()
    }
}

impl RelationGenerator for SqliteRelationGenerator {
    fn output_relations(&self) {
        // No-op: SQLite foreign keys are emitted inline in the `CREATE TABLE` statement
        // (see `inline_foreign_key_constraints`) since SQLite doesn't support adding
        // foreign key constraints via `ALTER TABLE` after the fact.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::{make_context, make_context_with_fk_mode};
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::relation::Relation;
    use schema_model::model::types::{BooleanMode, DatabaseType, RelationType};

    fn build_model_with_relation() -> (DatabaseModel, Table) {
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        (model, child)
    }

    #[test]
    fn output_relations_never_emits_alter_table() {
        let (model, _child) = build_model_with_relation();
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteRelationGenerator::new(ctx);
        generator.output_relations();

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    fn inline_foreign_key_constraints_renders_clause_for_relation() {
        let (model, child) = build_model_with_relation();
        let (ctx, _buffer) = make_context(model, DatabaseType::Sqlite);
        let generator = SqliteRelationGenerator::new(ctx);

        let clauses = generator.inline_foreign_key_constraints(&child);

        assert_eq!(clauses.len(), 1);
        assert!(clauses[0].contains("foreign key (parent_id) references parent(id)"));
        assert!(clauses[0].contains("on delete cascade"));
        assert!(!clauses[0].contains("alter table"));
    }

    #[test]
    fn inline_foreign_key_constraints_empty_when_table_has_no_relations() {
        let table = TableBuilder::new(None::<&str>, "solo").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteRelationGenerator::new(ctx);

        assert!(generator.inline_foreign_key_constraints(&table).is_empty());
    }

    #[test]
    fn inline_foreign_key_constraints_empty_when_foreign_key_mode_is_not_relations() {
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context_with_fk_mode(model, DatabaseType::Sqlite, ForeignKeyMode::None);

        let generator = SqliteRelationGenerator::new(ctx);

        assert!(generator.inline_foreign_key_constraints(&child).is_empty());
    }
}
