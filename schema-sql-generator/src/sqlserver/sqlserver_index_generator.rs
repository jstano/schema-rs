use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator, format_column_list};
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
        let mut options = Vec::new();

        if let Some(columns) = key.include() {
            options.push(format!("include ({})", format_column_list(columns)));
        }
        if key.is_compress() {
            // `compress` is a plain bool in the XML with no PAGE/ROW distinction; PAGE is
            // the higher-compression, generally-recommended default.
            options.push("with (data_compression = page)".to_string());
        }

        if options.is_empty() {
            None
        } else {
            Some(options.join(" "))
        }
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
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(output.contains("create unique index ix_users1 on dbo.users (email)"));
    }

    #[test]
    fn output_indexes_for_table_skips_when_no_indexes() {
        let table = TableBuilder::new(None::<&str>, "solo").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    fn output_indexes_for_table_renders_include_columns() {
        // Regression test for M1: `include` used to be parsed then thrown away.
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
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(
            output.contains("create index ix_t11 on dbo.t1 (id) include (name, code)"),
            "unexpected output: {output}"
        );
    }

    #[test]
    fn output_indexes_for_table_renders_data_compression() {
        // Regression test for M1: `compress` used to be parsed then thrown away.
        let index = Key::new_full(KeyType::Index, vec![KeyColumn::new("id")], false, true, false, None::<String>);
        let table = TableBuilder::new(None::<&str>, "t1").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(
            output.contains("create index ix_t11 on dbo.t1 (id) with (data_compression = page)"),
            "unexpected output: {output}"
        );
    }

    #[test]
    fn output_indexes_for_table_combines_include_and_compression() {
        let index = Key::new_full(KeyType::Index, vec![KeyColumn::new("id")], false, true, false, Some("code"));
        let table = TableBuilder::new(None::<&str>, "t1").add_index(index).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerIndexGenerator::new(ctx.clone());
        ctx.with_writer(|writer| {
            generator.output_indexes_for_table(writer, &table);
        });

        let output = buffer.contents();
        assert!(
            output.contains("create index ix_t11 on dbo.t1 (id) include (code) with (data_compression = page)"),
            "unexpected output: {output}"
        );
    }
}
