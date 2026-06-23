use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::sql_println;
use schema_model::model::table::Table;
use schema_model::model::types::LockEscalation;
use crate::sqlserver::sqlserver_column_constraint_generator::SqlServerColumnConstraintGenerator;
use crate::sqlserver::sqlserver_column_generator::SqlServerColumnGenerator;
use crate::sqlserver::sqlserver_index_generator::SqlServerIndexGenerator;
use crate::sqlserver::sqlserver_key_generator::SqlServerKeyGenerator;
use crate::sqlserver::sqlserver_table_constraint_generator::SqlServerTableConstraintGenerator;

pub struct SqlServerTableGenerator {
    context: GeneratorContext,
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
            context,
        }
    }
}

impl TableGenerator for SqlServerTableGenerator {
    fn output_tables(&self) {
        let database_model = self.context.settings().database_model();
        for schema in database_model.schemas() {
            for table in schema.tables() {
                self.output_table(table);
            }
        }
    }

    fn output_table(&self, table: &Table) {
        self.output_table_header(table);
        self.output_table_definition(table);
        self.output_table_footer(table);
        self.output_indexes(table);
        self.output_initial_data(table);
    }

    fn output_table_header(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_definition(&self, table: &Table) {
        self.table_generator.output_table_definition(table);
    }

    fn output_table_footer(&self, table: &Table) {
        self.table_generator.output_table_footer(table);

        match table.lock_escalation() {
            LockEscalation::Table | LockEscalation::Disable => {
                let lock_escalation_value = match table.lock_escalation() {
                    LockEscalation::Table => "TABLE",
                    LockEscalation::Disable => "DISABLE",
                    LockEscalation::Auto => return,
                };
                let separator = self.context.settings().statement_separator();
                self.context.with_writer(|writer| {
                    sql_println!(
                        writer,
                        "alter table {} set (lock_escalation = {}){}",
                        table.fully_qualified_table_name(),
                        lock_escalation_value,
                        separator
                    );
                    sql_println!(writer, "");
                });
            }
            LockEscalation::Auto => {}
        }
    }

    fn output_indexes(&self, table: &Table) {
        self.table_generator.output_indexes(table);
    }

    fn output_initial_data(&self, table: &Table) {
        self.table_generator.output_initial_data(table);
    }
}
