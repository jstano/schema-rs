use crate::common::generator_context::GeneratorContext;
use crate::common::key_generator::{DefaultKeyGenerator, KeyGenerator};
use crate::sqlite::sqlite_pk_support::inline_autoincrement_pk_column;
use schema_model::model::table::Table;

pub struct SqliteKeyGenerator {
    key_generator: DefaultKeyGenerator,
}

impl SqliteKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            key_generator: DefaultKeyGenerator::new(context.clone()),
        }
    }
}

impl KeyGenerator for SqliteKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        // When the primary key is a single Sequence/LongSequence column, it's declared
        // `integer primary key autoincrement` inline on the column (see `sqlite_pk_support`
        // and `SqliteColumnGenerator`); emitting the usual table-level `constraint pk_x
        // primary key (...)` as well would make SQLite reject the table for declaring more
        // than one primary key.
        let skip_primary_key = inline_autoincrement_pk_column(table).is_some();
        self.key_generator.key_constraints_filtered(table, skip_primary_key)
    }
}
