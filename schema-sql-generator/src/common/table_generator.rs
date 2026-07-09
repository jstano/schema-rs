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
        self.context.with_writer(|writer| {
            self.index_generator.output_indexes_for_table(writer, table);
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::column_generator::ColumnGenerator;
    use crate::common::index_generator::IndexGenerator;
    use crate::common::key_generator::KeyGenerator;
    use crate::common::table_constraint_generator::TableConstraintGenerator;
    use crate::common::test_support::make_context;
    use schema_model::builder::{SchemaBuilder, TableBuilder};
    use schema_model::model::column::Column;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::initial_data::InitialData;
    use schema_model::model::key::Key;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    struct NoopColumnGenerator;
    impl ColumnGenerator for NoopColumnGenerator {
        fn column_definitions(&self, _table: &Table) -> Vec<String> { Vec::new() }
        fn column_sql(&self, _table: &Table, _column: &Column) -> String { String::new() }
        fn column_options(&self, _table: &Table, _column: &Column) -> String { String::new() }
        fn default_value(&self, _table: &Table, _column: &Column) -> Option<String> { None }
    }

    struct NoopKeyGenerator;
    impl KeyGenerator for NoopKeyGenerator {
        fn key_constraints(&self, _table: &Table) -> Vec<String> { Vec::new() }
    }

    struct NoopColumnConstraintGenerator;
    impl ColumnConstraintGenerator for NoopColumnConstraintGenerator {
        fn column_check_constraints(&self, _table: &Table) -> Vec<String> { Vec::new() }
    }

    struct NoopTableConstraintGenerator;
    impl TableConstraintGenerator for NoopTableConstraintGenerator {
        fn table_check_constraints(&self, _table: &Table) -> Vec<String> { Vec::new() }
    }

    struct NoopIndexGenerator;
    impl IndexGenerator for NoopIndexGenerator {
        fn output_indexes(&self) {}
        fn output_indexes_for_table(&self, _writer: &mut crate::common::sql_writer::SqlWriter, _table: &Table) {}
        fn output_index(&self, _writer: &mut crate::common::sql_writer::SqlWriter, _statement_separator: &str, _table: &Table, _key_name: &str, _key: &Key) {}
        fn index_options(&self, _key: &Key) -> Option<String> { None }
    }

    fn make_generator(context: GeneratorContext) -> DefaultTableGenerator {
        DefaultTableGenerator::new(
            context,
            Box::new(NoopColumnGenerator),
            Box::new(NoopKeyGenerator),
            Box::new(NoopColumnConstraintGenerator),
            Box::new(NoopTableConstraintGenerator),
            Box::new(NoopIndexGenerator),
        )
    }

    #[test]
    fn output_initial_data_includes_rows_for_current_database_type_or_unspecified() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_initial_data(InitialData::new("insert into users values (1)", None))
            .add_initial_data(InitialData::new("insert into users values (2)", Some(DatabaseType::Sqlite)))
            .add_initial_data(InitialData::new("insert into users values (3)", Some(DatabaseType::Postgresql)))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = make_generator(ctx);
        generator.output_initial_data(&table);

        let output = buffer.contents();
        assert!(output.contains("insert into users values (1)"));
        assert!(output.contains("insert into users values (2)"));
        assert!(!output.contains("insert into users values (3)"));
    }

    #[test]
    fn output_initial_data_writes_nothing_when_table_has_no_rows() {
        let table = TableBuilder::new(None::<&str>, "users").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = make_generator(ctx);
        generator.output_initial_data(&table);

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    fn output_table_runs_all_steps_in_order() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_initial_data(InitialData::new("insert into users values (1)", None))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = make_generator(ctx);
        generator.output_table(&table);

        let output = buffer.contents();
        let header_pos = output.find("create table users").unwrap();
        let footer_pos = output.find(")").unwrap();
        let data_pos = output.find("insert into users values (1)").unwrap();
        assert!(header_pos < footer_pos);
        assert!(footer_pos < data_pos);
    }
}
