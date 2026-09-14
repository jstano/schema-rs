use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator};
use crate::common::sql_writer::SqlWriter;
use schema_model::model::key::Key;
use schema_model::model::table::Table;

pub struct SqliteIndexGenerator {
    index_generator: DefaultIndexGenerator,
}

impl SqliteIndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            index_generator: DefaultIndexGenerator::new(context),
        }
    }
}

impl IndexGenerator for SqliteIndexGenerator {
    fn output_indexes(&self) {
        // Self-dispatching, matching the other dialects (see
        // `PostgresIndexGenerator`/`SqlServerIndexGenerator`) so `index_options` below
        // (which warns on unsupported options) is actually reached for every index.
        self.index_generator.output_indexes_via(self);
    }

    fn output_indexes_for_table(&self, writer: &mut SqlWriter, table: &Table) {
        self.index_generator.output_indexes_for_table_via(self, writer, table);
    }

    fn output_index(&self, writer: &mut SqlWriter, statement_separator: &str, table: &Table, key_name: &str, key: &Key) {
        let index_options = self.index_options(key);
        self.index_generator.render_index(writer, statement_separator, table, key_name, key, index_options);
    }

    fn index_options(&self, key: &Key) -> Option<String> {
        // SQLite supports neither `INCLUDE` columns nor index compression - warn and ignore
        // rather than erroring, since one schema.xml is meant to target all three databases
        // (M1).
        if key.include().is_some() {
            eprintln!(
                "warning: index on ({}) has an include list, but SQLite does not support INCLUDE on indexes -- ignoring",
                key.columns_as_string()
            );
        }
        if key.is_compress() {
            eprintln!(
                "warning: index on ({}) has compress=\"true\", but SQLite does not support index compression -- ignoring",
                key.columns_as_string()
            );
        }

        None
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
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(output.contains("create unique index ix_users1 on users (email);"));
    }

    #[test]
    fn output_indexes_for_table_skips_when_no_indexes() {
        let table = TableBuilder::new(None::<&str>, "solo").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    fn output_indexes_for_table_ignores_include_and_compress_since_sqlite_has_no_equivalent() {
        // Regression test for M1: `include`/`compress` have no SQLite equivalent - neither
        // must appear in the output, but the index itself must still be generated rather
        // than erroring/panicking.
        let index = Key::new_full(KeyType::Index, vec![KeyColumn::new("id")], false, true, false, Some("name,code"));
        let table = TableBuilder::new(None::<&str>, "t1").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert_eq!(output.trim(), "create index ix_t11 on t1 (id);");
    }
}
