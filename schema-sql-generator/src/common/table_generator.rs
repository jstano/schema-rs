use crate::common::generator_context::GeneratorContext;
use schema_model::model::table::Table;
use crate::{sql_newline, sql_println};

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
    // column_generator: ColumnGenerator,
    // key_generator: KeyGenerator,
    // column_constraint_generator: ColumnConstraintGenerator,
    // table_constraint_generator: TableConstraintGenerator,
    // index_generator: IndexGenerator,
}

impl DefaultTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
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
    }

    fn output_table_footer(&self, table: &Table) {
        self.context.with_writer(|writer| {
            sql_println!(writer, "){}", self.context.settings().statement_separator());
            sql_newline!(writer);
        });
    }

    fn output_indexes(&self, table: &Table) {
        // self.index_generator.output_indexes(table);
    }

    fn output_initial_data(&self, table: &Table) {
        /*
        let initialDataList = table.initial_data()
            .iter().map(it => it.getDatabaseType() == null || it.getDatabaseType() == databaseType)
            .toList();

        if (!initialDataList.isEmpty()) {
            initialDataList.forEach(initialData -> {
                sqlWriter.println(initialData.getSql() + statementSeparator);
            });

            sqlWriter.println();
        }
        */
    }
}
