use crate::common::generator_context::GeneratorContext;
use crate::common::sql_writer::SqlWriter;
use schema_model::model::key::Key;
use schema_model::model::table::Table;

const IX_PREFIX: &str = "ix_";

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

impl IndexGenerator for DefaultIndexGenerator {
    fn output_indexes(&self) {
        let database_model = self.context.settings().database_model();

        self.context.with_writer(|writer| {
            database_model.schemas().iter().for_each(|schema| {
                schema.tables().iter().for_each(|table| {
                    self.output_indexes_for_table(writer, table);
                });
            });
        });
    }

    fn output_indexes_for_table(&self, writer: &mut SqlWriter, table: &Table) {
        if !table.indexes().is_empty() {
            let max_key_name_length = self
                .context()
                .settings()
                .database_type()
                .max_key_name_length();

            for (key_index, key) in table
                .indexes()
                .iter()
                .filter(|key| key.is_index())
                .enumerate()
            {
                let suffix_str = (key_index + 1).to_string();
                let mut key_name = format!("{}{}{}", IX_PREFIX, table.name(), suffix_str).to_lowercase();

                if key_name.len() > max_key_name_length {
                    // Reserve space for the *actual* suffix length, not a hard-coded
                    // budget - a table with >=10 indexes needs a 2-digit suffix, and a
                    // fixed 4-char reservation (3-char prefix + 1-digit suffix) would
                    // produce an identifier over the length limit.
                    let max_name_len = max_key_name_length.saturating_sub(IX_PREFIX.len() + suffix_str.len());
                    let truncated = table
                        .name()
                        .chars()
                        .take(max_name_len)
                        .collect::<String>();
                    key_name = format!("{}{}{}", IX_PREFIX, truncated, suffix_str).to_lowercase();
                }

                self.output_index(
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

    fn output_index(
        &self,
        writer: &mut SqlWriter,
        statement_separator: &str,
        table: &Table,
        key_name: &str,
        key: &Key,
    ) {
        let index_options = self.index_options(key);
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

    fn index_options(&self, _key: &Key) -> Option<String> {
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
