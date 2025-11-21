use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::{DefaultIndexGenerator, IndexGenerator};
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
