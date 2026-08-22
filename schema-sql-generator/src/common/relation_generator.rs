use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::relation::Relation;
use schema_model::model::table::Table;
use schema_model::model::types::RelationType;

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
        let database_model = self.context.settings().database_model();

        for (relation_index, relation) in table.relations().iter().enumerate() {
            let relation_name = self.relation_constraint_name(table, relation_index);
            self.output_relation(writer, &relation_name, database_model, table, relation);
        }
    }

    /// Builds the constraint name for the given relation, truncating the table-name
    /// portion so the identifier stays within the target database's max key name length.
    pub fn relation_constraint_name(&self, table: &Table, relation_index: usize) -> String {
        let database_type = self.context.settings().database_type();
        let max_key_name_length = database_type.max_key_name_length();
        let table_name = table.name();
        let suffix_str = (relation_index + 1).to_string();
        let mut relation_name = format!("{}{}{}", FK_PREFIX, table_name, suffix_str);

        if relation_name.len() > max_key_name_length {
            // Reserve space for the *actual* suffix length, not a hard-coded single
            // digit - a table with >=10 relations needs a 2-digit suffix, and reserving
            // only 1 char for it would produce an identifier over the length limit.
            // Truncate by char, not byte index, so multi-byte UTF-8 table names don't
            // panic ("byte index N is not a char boundary").
            let available = max_key_name_length.saturating_sub(FK_PREFIX.len() + suffix_str.len());
            let truncated_table_name: String = table_name.chars().take(available).collect();
            relation_name = format!("{}{}{}", FK_PREFIX, truncated_table_name, suffix_str);
        }

        relation_name.to_lowercase()
    }

    fn output_relation(&self,
                       writer: &mut SqlWriter,
                       relation_name: &str,
                       database_model: &DatabaseModel,
                       table: &Table,
                       relation: &Relation) {
        let operation = self.relation_operation_type(relation.relation_type());
        let database_type = self.context.settings().database_type();
        let to_table = database_model.find_table_by_qualified_name(relation.to_table_name());

        writer.print(format!("alter table {}", table.fully_qualified_table_name(database_type)).as_str());
        writer.print(" add constraint ");
        writer.print(relation_name);
        writer.print(" foreign key (");
        writer.print(relation.from_column_name());
        writer.print(") references ");
        writer.print(to_table.fully_qualified_table_name(database_type).as_str());
        writer.print("(");
        writer.print(relation.to_column_name());
        writer.print(") on delete ");
        writer.print(operation);
        writer.println(self.context().settings().statement_separator());
    }

    pub fn relation_operation_type(&self, relation_type: RelationType) -> &str {
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
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultRelationGenerator::new(ctx);
        generator.output_relations();

        let output = buffer.contents();
        assert!(output.contains("add constraint "));
        let constraint_start = output.find("add constraint ").unwrap() + "add constraint ".len();
        let constraint_name = &output[constraint_start..].split_whitespace().next().unwrap();
        assert!(constraint_name.len() <= 63, "constraint name '{}' exceeds postgres's 63 char limit", constraint_name);
    }

    #[test]
    fn relation_constraint_name_truncates_multi_byte_table_name_without_panicking() {
        // Regression test: byte-index slicing panics ("not a char boundary") on
        // multi-byte UTF-8 once truncation kicks in; truncating by char must not.
        let long_table_name = "语".repeat(70);
        let table = TableBuilder::new(None::<&str>, long_table_name.as_str()).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultRelationGenerator::new(ctx);
        let name = generator.relation_constraint_name(&table, 0);
        assert!(name.starts_with("fk_"));
        // max_key_name_length is a character budget (SQL Server identifiers are
        // nvarchar), not a byte budget, so compare char count, not `str::len()` (bytes)
        // - which would always be inflated for multi-byte UTF-8 like "语".
        assert!(name.chars().count() <= 32);
    }

    #[test]
    fn relation_constraint_name_stays_within_limit_for_double_digit_relation_index() {
        // Regression test: reserving only 1 char for the suffix (instead of measuring
        // its actual length) produced a 33-char identifier here, one over SQL Server's
        // 32-char limit, once the relation index reaches double digits.
        let long_table_name = "a".repeat(40);
        let table = TableBuilder::new(None::<&str>, long_table_name.as_str()).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultRelationGenerator::new(ctx);
        // relation_index 9 -> suffix "10" (double digit)
        let name = generator.relation_constraint_name(&table, 9);
        assert!(name.ends_with("10"));
        assert!(name.len() <= 32, "constraint name '{}' exceeds SQL Server's 32 char limit", name);
    }
}
