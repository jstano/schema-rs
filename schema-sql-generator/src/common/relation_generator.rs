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
        let to_table = self.find_table(relation.to_table_name(), database_model);

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
            RelationType::SetNull => {"setnull"}
            RelationType::DoNothing => {"no action"}
        }
    }

    fn find_table<'a>(&self, to_table_name: &str, database_model: &'a DatabaseModel) -> &'a Table {
        let parts: Vec<&str> = to_table_name.split('.').collect();
        let (schema, table_name) = if parts.len() == 2 {
            (Some(parts[0]), parts[1])
        } else {
            (None, to_table_name)
        };

        database_model.find_table(schema, table_name)
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
