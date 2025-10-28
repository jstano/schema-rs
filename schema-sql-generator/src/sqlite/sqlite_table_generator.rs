use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::sqlite::sqlite_column_constraint_generator::SqliteColumnConstraintGenerator;
use crate::sqlite::sqlite_column_generator::SqliteColumnGenerator;
use crate::sqlite::sqlite_index_generator::SqliteIndexGenerator;
use crate::sqlite::sqlite_key_generator::SqliteKeyGenerator;
use crate::sqlite::sqlite_table_constraint_generator::SqliteTableConstraintGenerator;
use schema_model::model::table::Table;

pub struct SqliteTableGenerator {
    context: GeneratorContext,
    table_generator: DefaultTableGenerator,
}

impl SqliteTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(
                context.clone(),
                Box::new(SqliteColumnGenerator::new(context.clone())),
                Box::new(SqliteKeyGenerator::new(context.clone())),
                Box::new(SqliteColumnConstraintGenerator::new(context.clone())),
                Box::new(SqliteTableConstraintGenerator::new(context.clone())),
                Box::new(SqliteIndexGenerator::new(context.clone())),
            ),
            context,
        }
    }
}

impl TableGenerator for SqliteTableGenerator {
    fn output_tables(&self) {
        self.table_generator.output_tables();
    }

    fn output_table(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_header(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_definition(&self, table: &Table) {
        self.table_generator.output_table_definition(table);
    }

    fn output_table_footer(&self, table: &Table) {
        self.table_generator.output_table_footer(table);
    }

    fn output_indexes(&self, table: &Table) {
        self.table_generator.output_indexes(table);
    }

    fn output_initial_data(&self, table: &Table) {
        self.table_generator.output_initial_data(table);
    }
}
