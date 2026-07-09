use schema_model::model::database_model::DatabaseModel;
use schema_model::model::relation::Relation;
use schema_model::model::table::Table;
use schema_model::model::types::RelationType;
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;

const FK_PREFIX: &str = "fk_";

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

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn output_relation_for_table(&self, writer: &mut SqlWriter, table: &Table) {
        let database_type = self.context.settings().database_type();
        let database_model = self.context.settings().database_model();
        let max_key_name_length = database_type.max_key_name_length();
        let table_name = table.name();
        let relations = table.relations();

        for (relation_index, relation) in relations.iter().enumerate() {
            let mut relation_name = format!("{}{}{}", FK_PREFIX, table_name, relation_index + 1);

            if relation_name.len() > max_key_name_length {
                let truncated_table_name_len = max_key_name_length - FK_PREFIX.len() - 1; // leave space for the index
                let truncated_table_name = &table_name[..truncated_table_name_len.min(table_name.len())];
                relation_name = format!("{}{}{}", FK_PREFIX, truncated_table_name, relation_index + 1);
            }

            self.output_relation(writer, &relation_name.to_lowercase(), database_model, table, relation);
        }
    }

    fn output_relation(&self,
                       writer: &mut SqlWriter,
                       relation_name: &str,
                       database_model: &DatabaseModel,
                       table: &Table,
                       relation: &Relation) {
        let operation = self.relation_operation_type(relation.relation_type());
        let to_table = database_model.find_table_by_qualified_name(relation.to_table_name());

        writer.print(format!("alter table {}", table.fully_qualified_table_name()).as_str());
        writer.print(" add constraint ");
        writer.print(relation_name);
        writer.print(" foreign key (");
        writer.print(relation.from_column_name());
        writer.print(") references ");
        writer.print(to_table.fully_qualified_table_name().as_str());
        writer.print("(");
        writer.print(relation.to_column_name());
        writer.print(") on delete ");
        writer.print(operation);
        writer.println(self.context().settings().statement_separator());
    }

    fn relation_operation_type(&self, relation_type: RelationType) -> &str {
        match relation_type {
            RelationType::Cascade => {"cascade"}
            RelationType::Enforce => {"no action"}
            RelationType::SetNull => {"set null"}
            RelationType::DoNothing => {"no action"}
        }
    }

}

impl RelationGenerator for DefaultRelationGenerator {
    fn output_relations(&self) {
        let database_model = self.context.settings().database_model();
        let has_relations = database_model.all_tables().iter().any(|table| {!table.relations().is_empty()});

        if has_relations {
            self.context.with_writer(|writer| {
                writer.println("/* relations */");

                database_model.all_tables().iter().filter(|table| {
                    !table.relations().is_empty()
                }).for_each(|table| {
                    self.output_relation_for_table(writer, table);
                });

                writer.newline();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_relations_truncates_generated_constraint_name_for_long_table_names() {
        // Postgres caps key names at 63 chars; a long table name plus the "fk_" prefix and
        // trailing index would overflow that, so the generator must truncate the table name
        // portion rather than emit an invalid identifier.
        let long_table_name = "a".repeat(70);
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(None::<&str>, long_table_name.as_str())
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(Relation::new("parent", "id", long_table_name.as_str(), "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultRelationGenerator::new(ctx);
        generator.output_relations();

        let output = buffer.contents();
        assert!(output.contains("add constraint "));
        let constraint_start = output.find("add constraint ").unwrap() + "add constraint ".len();
        let constraint_name = &output[constraint_start..].split_whitespace().next().unwrap();
        assert!(constraint_name.len() <= 63, "constraint name '{}' exceeds postgres's 63 char limit", constraint_name);
    }
}
