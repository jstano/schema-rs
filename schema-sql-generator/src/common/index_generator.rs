use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::key::Key;
use schema_model::model::table::Table;
use schema_model::naming::index_name;

pub trait IndexGenerator {
    fn output_indexes(&self);

    fn output_indexes_for_table(&self, writer: &mut SqlWriter, table: &Table);

    fn output_index(
        &self,
        writer: &mut SqlWriter,
        statement_separator: &str,
        table: &Table,
        key_name: &str,
        key: &Key,
    );

    fn index_options(&self, key: &Key) -> Option<String>;
}

pub struct DefaultIndexGenerator {
    context: GeneratorContext,
}

impl DefaultIndexGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl DefaultIndexGenerator {
    /// Same iteration logic as `output_indexes`, but dispatches each table (and,
    /// transitively, each index) through `generator` rather than `self` - see
    /// `DefaultFunctionGenerator::output_functions_via` for why: a dialect wrapper (e.g.
    /// `SqlServerIndexGenerator`) that overrides `index_options` but delegates
    /// `output_indexes` straight to this struct would otherwise never see its own override
    /// invoked, because Rust has no virtual dispatch on concrete types. Callers that need
    /// their override honored should pass `self` (as a `&dyn IndexGenerator`) here instead
    /// of calling `output_indexes` directly.
    pub fn output_indexes_via(&self, generator: &dyn IndexGenerator) {
        let database_model = self.context.settings().database_model();

        self.context.with_writer(|writer| {
            database_model.schemas().iter().for_each(|schema| {
                schema.tables().iter().for_each(|table| {
                    generator.output_indexes_for_table(writer, table);
                });
            });
        });
    }

    /// Same as `output_indexes_via`, one table at a time - see that method for why
    /// dispatching through `generator` (rather than calling `self.output_index` directly)
    /// matters.
    pub fn output_indexes_for_table_via(&self, generator: &dyn IndexGenerator, writer: &mut SqlWriter, table: &Table) {
        if !table.indexes().is_empty() {
            let database_type = self.context().settings().database_type();

            for (key_index, key) in table
                .indexes()
                .iter()
                .filter(|key| key.is_index())
                .enumerate()
            {
                let key_name = index_name(database_type, table.name(), key_index + 1);

                generator.output_index(
                    writer,
                    self.context().settings().statement_separator(),
                    table,
                    key_name.as_str(),
                    key,
                );
            }

            writer.newline();
        }
    }

    /// Renders one `create index` statement given an already-computed `index_options` -
    /// shared by `DefaultIndexGenerator::output_index` and every dialect override, so each
    /// only has to supply its own `index_options` before calling back into this.
    pub fn render_index(
        &self,
        writer: &mut SqlWriter,
        statement_separator: &str,
        table: &Table,
        key_name: &str,
        key: &Key,
        index_options: Option<String>,
    ) {
        let fully_qualified_table_name = table.fully_qualified_table_name(self.context().settings().database_type());
        let index_columns = key
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>()
            .join(", ");

        if let Some(index_options) = index_options {
            writer.println(
                format!(
                    "create {}index {} on {} ({}) {}{}",
                    if key.is_unique() { "unique " } else { "" },
                    key_name,
                    fully_qualified_table_name,
                    index_columns,
                    index_options,
                    statement_separator
                )
                    .as_str(),
            );
        } else {
            writer.println(
                format!(
                    "create {}index {} on {} ({}){}",
                    if key.is_unique() { "unique " } else { "" },
                    key_name,
                    fully_qualified_table_name,
                    index_columns,
                    statement_separator
                )
                    .as_str(),
            );
        }
    }
}

impl IndexGenerator for DefaultIndexGenerator {
    fn output_indexes(&self) {
        self.output_indexes_via(self);
    }

    fn output_indexes_for_table(&self, writer: &mut SqlWriter, table: &Table) {
        self.output_indexes_for_table_via(self, writer, table);
    }

    fn output_index(
        &self,
        writer: &mut SqlWriter,
        statement_separator: &str,
        table: &Table,
        key_name: &str,
        key: &Key,
    ) {
        let index_options = self.index_options(key);
        self.render_index(writer, statement_separator, table, key_name, key, index_options);
    }

    fn index_options(&self, _key: &Key) -> Option<String> {
        None
    }
}

/// Formats a comma-separated column list from XML (e.g. `"name,code"`) as SQL wants it
/// (e.g. `"name, code"`), trimming any incidental whitespace around each name.
pub(crate) fn format_column_list(columns: &str) -> String {
    columns.split(',').map(|c| c.trim()).collect::<Vec<_>>().join(", ")
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
    fn output_indexes_for_table_truncates_multi_byte_table_name_without_panicking() {
        // Regression test: `.chars().take(n)` was already char-safe here, but the budget
        // it was given (`max_key_name_length - 4`) could still be wrong; this guards the
        // overall path stays panic-free for multi-byte UTF-8 table names.
        let long_table_name = "语".repeat(70);
        let idx = Key::new(KeyType::Index, vec![KeyColumn::new("name")]);
        let table = TableBuilder::new(None::<&str>, long_table_name.as_str())
            .add_index(idx)
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultIndexGenerator::new(ctx);
        generator.output_indexes();

        let output = buffer.contents();
        assert!(output.contains("create index"));
    }

    #[test]
    fn index_name_stays_within_limit_for_double_digit_index() {
        // Regression test: the old hard-coded `saturating_sub(4)` budget reserved only 1
        // char for the numeric suffix; once a table has >=10 indexes, the suffix needs 2
        // digits and the old logic produced an identifier one char over the limit.
        let long_table_name = "a".repeat(40);
        let mut table_builder = TableBuilder::new(None::<&str>, long_table_name.as_str());
        for i in 0..10 {
            table_builder = table_builder.add_index(Key::new(KeyType::Index, vec![KeyColumn::new(format!("col{i}"))]));
        }
        let table = table_builder.build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultIndexGenerator::new(ctx);
        generator.output_indexes();

        let output = buffer.contents();
        for line in output.lines().filter(|l| l.starts_with("create index")) {
            let name = line.split_whitespace().nth(2).unwrap();
            assert!(name.len() <= 32, "index name '{}' exceeds SQL Server's 32 char limit", name);
        }
        assert!(output.contains("ix_"));
    }
}
