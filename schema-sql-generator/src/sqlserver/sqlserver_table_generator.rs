use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use schema_model::model::table::Table;
use crate::sqlserver::sqlserver_column_constraint_generator::SqlServerColumnConstraintGenerator;
use crate::sqlserver::sqlserver_column_generator::SqlServerColumnGenerator;
use crate::sqlserver::sqlserver_index_generator::SqlServerIndexGenerator;
use crate::sqlserver::sqlserver_key_generator::SqlServerKeyGenerator;
use crate::sqlserver::sqlserver_table_constraint_generator::SqlServerTableConstraintGenerator;

pub struct SqlServerTableGenerator {
    table_generator: DefaultTableGenerator,
}

impl SqlServerTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(
                context.clone(),
                Box::new(SqlServerColumnGenerator::new(context.clone())),
                Box::new(SqlServerKeyGenerator::new(context.clone())),
                Box::new(SqlServerColumnConstraintGenerator::new(context.clone())),
                Box::new(SqlServerTableConstraintGenerator::new(context.clone())),
                Box::new(SqlServerIndexGenerator::new(context.clone())),
            ),
        }
    }
}

impl TableGenerator for SqlServerTableGenerator {
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
