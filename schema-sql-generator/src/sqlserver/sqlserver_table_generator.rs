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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_table_renders_header_and_columns() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "name", ColumnType::Varchar).length(50).required(true).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_header(&table);
        generator.output_table_definition(&table);
        generator.output_table_footer(&table);

        let output = buffer.contents();
        assert!(output.contains("create table users"));
        assert!(output.contains("id integer identity(1,1)"));
        assert!(output.contains("name nvarchar(50)"));
    }

    #[test]
    fn output_table_footer_emits_lock_escalation_when_table() {
        let table = TableBuilder::new(None::<&str>, "users")
            .lock_escalation(LockEscalation::Table)
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_footer(&table);

        assert!(buffer.contents().contains("alter table users set (lock_escalation = TABLE)"));
    }

    #[test]
    fn output_table_footer_omits_lock_escalation_when_auto() {
        let table = TableBuilder::new(None::<&str>, "users")
            .lock_escalation(LockEscalation::Auto)
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_footer(&table);

        assert!(!buffer.contents().contains("lock_escalation"));
    }
}
