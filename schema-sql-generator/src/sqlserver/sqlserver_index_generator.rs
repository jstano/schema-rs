use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator};
use crate::common::sql_writer::SqlWriter;
use schema_model::model::key::Key;
use schema_model::model::table::Table;

pub struct SqlServerIndexGenerator {
    index_generator: DefaultIndexGenerator,
}

impl SqlServerIndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            index_generator: DefaultIndexGenerator::new(context),
        }
    }
}

impl IndexGenerator for SqlServerIndexGenerator {
    fn output_indexes(&self) {
        self.index_generator.output_indexes();
    }

    fn output_indexes_for_table(&self, writer: &mut SqlWriter, table: &Table) {
        self.index_generator.output_indexes_for_table(writer, table);
    }

    fn output_index(&self, writer: &mut SqlWriter, statement_separator: &str, table: &Table, key_name: &str, key: &Key) {
        self.index_generator.output_index(writer, statement_separator, table, key_name, key);
    }

    fn index_options(&self, key: &Key) -> Option<String> {
        self.index_generator.index_options(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::key::KeyColumn;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, KeyType};

    #[test]
    fn output_indexes_for_table_renders_unique_index() {
        let index = Key::new_full(KeyType::Index, vec![KeyColumn::new("email")], false, false, true, None::<String>);
        let table = TableBuilder::new(None::<&str>, "users").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(output.contains("create unique index ix_users1 on users (email)"));
    }

    #[test]
    fn output_indexes_for_table_skips_when_no_indexes() {
        let table = TableBuilder::new(None::<&str>, "solo").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        assert_eq!(buffer.contents(), "");
    }
}
