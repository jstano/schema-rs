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
                let mut key_name = format!("{}{}{}", IX_PREFIX, table.name(), key_index + 1);

                if key_name.len() > max_key_name_length {
                    let max_name_len = max_key_name_length.saturating_sub(4); // match Java logic
                    let truncated = table
                        .name()
                        .chars()
                        .take(max_name_len.min(table.name().len()))
                        .collect::<String>();
                    key_name = format!("{}{}{}", IX_PREFIX, truncated, key_index + 1);
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
        let index_columns = key
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>()
            .join(", ");

        if index_options.is_some() {
            writer.println(
                format!(
                    "create {}index {} on {} ({}) {}{}",
                    if key.is_unique() { "unique " } else { "" },
                    key_name,
                    table.name(),
                    index_columns,
                    index_options.unwrap(),
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
                    table.name(),
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
