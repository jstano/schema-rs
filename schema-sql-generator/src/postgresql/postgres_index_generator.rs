use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator, format_column_list};
use crate::common::sql_writer::SqlWriter;
use schema_model::model::key::Key;
use schema_model::model::table::Table;

pub struct PostgresIndexGenerator {
    index_generator: DefaultIndexGenerator,
}

impl PostgresIndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            index_generator: DefaultIndexGenerator::new(context),
        }
    }
}

impl IndexGenerator for PostgresIndexGenerator {
    fn output_indexes(&self) {
        // Self-dispatching (via `output_indexes_via(self)`, not a plain delegation to
        // `DefaultIndexGenerator::output_indexes`) so every index routes through *this*
        // type's `index_options` override below - see `render_index`/`output_indexes_via`
        // for the same static-dispatch trap this avoids (M1).
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
        // PostgreSQL has supported `INCLUDE (...)` on indexes since v11. `compress` has no
        // PostgreSQL equivalent - warn and ignore it rather than erroring, since one
        // schema.xml is meant to target all three databases (M1).
        if key.is_compress() {
            eprintln!(
                "warning: index on ({}) has compress=\"true\", but PostgreSQL does not support index compression -- ignoring",
                key.columns_as_string()
            );
        }

        let mut options = Vec::new();
        if let Some(columns) = key.include() {
            options.push(format!("include ({})", format_column_list(columns)));
        }
        if let Some(filter) = key.filter() {
            options.push(format!("where {}", filter));
        }

        if options.is_empty() { None } else { Some(options.join(" ")) }
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
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(output.contains("create unique index ix_users1 on public.users (email);"));
    }

    #[test]
    fn output_indexes_for_table_skips_when_no_indexes() {
        let table = TableBuilder::new(None::<&str>, "solo").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    fn output_indexes_for_table_renders_include_columns() {
        // Regression test for M1: `include` used to be parsed then thrown away. PostgreSQL
        // has supported INCLUDE on indexes since v11, unlike SQLite.
        let index = Key::new_full(
            KeyType::Index,
            vec![KeyColumn::new("id")],
            false,
            false,
            false,
            Some("name,code"),
        );
        let table = TableBuilder::new(None::<&str>, "t1").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(
            output.contains("create index ix_t11 on public.t1 (id) include (name, code);"),
            "unexpected output: {output}"
        );
    }

    #[test]
    fn output_indexes_for_table_renders_where_after_include() {
        let index = Key::new_full_with_filter(
            KeyType::Index,
            vec![KeyColumn::new("parent_id")],
            false,
            false,
            true,
            Some("code"),
            Some("parent_id is not null"),
        );
        let table = TableBuilder::new(None::<&str>, "t1").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(
            output.contains("create unique index ix_t11 on public.t1 (parent_id) include (code) where parent_id is not null;"),
            "unexpected output: {output}"
        );
    }

    #[test]
    fn output_indexes_for_table_ignores_compress_since_postgres_has_no_equivalent() {
        // `compress` has no PostgreSQL rendering (M1) - it must not appear in the output,
        // but the index itself must still be generated rather than erroring/panicking.
        let index = Key::new_full(KeyType::Index, vec![KeyColumn::new("id")], false, true, false, None::<String>);
        let table = TableBuilder::new(None::<&str>, "t1").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = PostgresIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert_eq!(output.trim(), "create index ix_t11 on public.t1 (id);");
    }
}
