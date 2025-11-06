use crate::common::column_constraint_generator::ColumnConstraintGenerator;
use crate::common::column_generator::ColumnGenerator;
use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::IndexGenerator;
use crate::common::key_generator::KeyGenerator;
use crate::common::table_constraint_generator::TableConstraintGenerator;
use crate::{sql_newline, sql_print, sql_println};
use schema_model::model::table::Table;

pub trait TableGenerator {
    fn output_tables(&self);
    fn output_table(&self, table: &Table);
    fn output_table_header(&self, table: &Table);
    fn output_table_definition(&self, table: &Table);
    fn output_table_footer(&self, table: &Table);
    fn output_indexes(&self, table: &Table);
    fn output_initial_data(&self, table: &Table);
}

pub struct DefaultTableGenerator {
    context: GeneratorContext,
    column_generator: Box<dyn ColumnGenerator>,
    key_generator: Box<dyn KeyGenerator>,
    column_constraint_generator: Box<dyn ColumnConstraintGenerator>,
    table_constraint_generator: Box<dyn TableConstraintGenerator>,
    index_generator: Box<dyn IndexGenerator>,
}

impl DefaultTableGenerator {
    pub fn new(
        context: GeneratorContext,
        column_generator: Box<dyn ColumnGenerator>,
        key_generator: Box<dyn KeyGenerator>,
        column_constraint_generator: Box<dyn ColumnConstraintGenerator>,
        table_constraint_generator: Box<dyn TableConstraintGenerator>,
        index_generator: Box<dyn IndexGenerator>,
    ) -> Self {
        Self {
            column_generator,
            key_generator,
            column_constraint_generator,
            table_constraint_generator,
            index_generator,
            context,
        }
    }
}

impl TableGenerator for DefaultTableGenerator {
    fn output_tables(&self) {
        for schema in self.context.settings().database_model().schemas() {
            for table in schema.tables() {
                self.output_table(table);
            };
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
        self.context.with_writer(|writer| {
            let fully_qualified_table_name = table.fully_qualified_table_name();
            sql_println!(writer, "/* {} */", fully_qualified_table_name);
            sql_println!(writer, "create table {}", fully_qualified_table_name);
            sql_println!(writer, "(");
        });
    }

    fn output_table_definition(&self, table: &Table) {
        let table_definitions: Vec<String> = self.column_generator.column_definitions(table)
            .into_iter()
            .chain(self.key_generator.key_constraints(table))
            .chain(self.column_constraint_generator.column_check_constraints(table))
            .chain(self.table_constraint_generator.table_check_constraints(table))
            .collect();

        self.context.with_writer(|writer| {
            for (i, sql) in table_definitions.iter().enumerate() {
                sql_print!(writer, "{}", sql);

                if i < table_definitions.len() - 1 {
                    sql_print!(writer, ",");
                }

                sql_newline!(writer);
            }
        });
    }

    fn output_table_footer(&self, _table: &Table) {
        self.context.with_writer(|writer| {
            sql_println!(writer, "){}", self.context.settings().statement_separator());
            sql_newline!(writer);
        });
    }

    fn output_indexes(&self, table: &Table) {
        self.index_generator.output_indexes_for_table(table);
    }

    fn output_initial_data(&self, table: &Table) {
        let initial_data = table.initial_data()
            .iter()
            .filter(|it| { it.database_type().is_none() || it.database_type().unwrap() == self.context.settings().database_type() })
            .collect::<Vec<_>>();

        if initial_data.len() > 0 {
            self.context.with_writer(|writer| {
                initial_data.iter().for_each(|it| {
                    sql_println!(writer, "{}{}", it.sql(), self.context.settings().statement_separator());
                });

                writer.newline();
            });
        }
    }
}
