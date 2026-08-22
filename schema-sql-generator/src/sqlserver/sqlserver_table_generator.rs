use crate::common::generator_context::GeneratorContext;
use crate::common::sql_string::escape_sql_literal;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::sql_println;
use crate::sqlserver::sqlserver_column_constraint_generator::SqlServerColumnConstraintGenerator;
use crate::sqlserver::sqlserver_column_generator::SqlServerColumnGenerator;
use crate::sqlserver::sqlserver_index_generator::SqlServerIndexGenerator;
use crate::sqlserver::sqlserver_key_generator::SqlServerKeyGenerator;
use crate::sqlserver::sqlserver_table_constraint_generator::SqlServerTableConstraintGenerator;
use schema_model::model::table::Table;
use schema_model::model::types::LockEscalation;

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
        // The legacy Java codegen tool's SQL Server output uses an existence-checked drop
        // (`if exists (select ... from dbo.sysobjects ...)`) and `GO` batches rather than the
        // `drop table if exists ...;` form the other dialects share, so this is a full override
        // rather than a delegation to `DefaultTableGenerator::output_table_header`.
        let database_type = self.context.settings().database_type();
        let separator = self.context.settings().statement_separator().to_string();
        let fully_qualified_table_name = table.fully_qualified_table_name(database_type);
        let table_name = table.name();

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {} */", table_name);
            sql_println!(writer, "if exists (select name from dbo.sysobjects where name = '{}' and type = 'U')", escape_sql_literal(table_name));
            sql_println!(writer, "drop table {}{}", fully_qualified_table_name, separator);
            sql_println!(writer, "");
            sql_println!(writer, "create table {}", fully_qualified_table_name);
            sql_println!(writer, "(");
        });
    }

    fn output_table_definition(&self, table: &Table) {
        self.table_generator.output_table_definition(table);
    }

    fn output_table_footer(&self, table: &Table) {
        self.table_generator.output_table_footer(table);

        // Unlike Postgres/SQLite, the legacy Java tool always emits the lock_escalation clause,
        // including for the default `Auto` setting.
        let lock_escalation_value = match table.lock_escalation() {
            LockEscalation::Auto => "auto",
            LockEscalation::Disable => "disable",
            LockEscalation::Table => "table",
        };
        let separator = self.context.settings().statement_separator();
        let fully_qualified_table_name = table.fully_qualified_table_name(self.context.settings().database_type());
        self.context.with_writer(|writer| {
            sql_println!(
                writer,
                "alter table {} set (lock_escalation = {}){}",
                fully_qualified_table_name,
                lock_escalation_value,
                separator
            );
            sql_println!(writer, "");
        });
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
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_header(&table);
        generator.output_table_definition(&table);
        generator.output_table_footer(&table);

        let output = buffer.contents();
        assert!(output.contains("/* users */"));
        assert!(output.contains("if exists (select name from dbo.sysobjects where name = 'users' and type = 'U')"));
        assert!(output.contains("drop table dbo.users\nGO"));
        assert!(output.contains("create table dbo.users"));
        assert!(output.contains("id integer identity(1,1)"));
        assert!(output.contains("name nvarchar(50)"));
    }

    #[test]
    fn output_table_header_escapes_single_quote_in_table_name() {
        // Regression test: an unescaped embedded quote would break the generated
        // sysobjects existence-check SQL string literal.
        let table = TableBuilder::new(None::<&str>, "o'brien").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_header(&table);

        let output = buffer.contents();
        assert!(output.contains("where name = 'o''brien' and type = 'U'"));
    }

    #[test]
    fn output_table_footer_emits_lock_escalation_when_table() {
        let table = TableBuilder::new(None::<&str>, "users")
            .lock_escalation(LockEscalation::Table)
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_footer(&table);

        assert!(buffer.contents().contains("alter table dbo.users set (lock_escalation = table)\nGO"));
    }

    #[test]
    fn output_table_footer_emits_lock_escalation_when_auto() {
        let table = TableBuilder::new(None::<&str>, "users")
            .lock_escalation(LockEscalation::Auto)
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_footer(&table);

        assert!(buffer.contents().contains("alter table dbo.users set (lock_escalation = auto)\nGO"));
    }

    #[test]
    fn output_table_footer_emits_lock_escalation_when_disable() {
        let table = TableBuilder::new(None::<&str>, "users")
            .lock_escalation(LockEscalation::Disable)
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_footer(&table);

        assert!(buffer.contents().contains("alter table dbo.users set (lock_escalation = disable)\nGO"));
    }

    #[test]
    fn output_table_footer_qualifies_lock_escalation_with_non_default_schema() {
        // Regression test: the lock_escalation ALTER used the bare table name while every
        // other statement for the table (create/drop/sysobjects check) used the fully
        // qualified name - for a non-default schema this would alter the wrong object
        // (SQL Server resolves an unqualified name via the connection's default schema,
        // not necessarily the table's declared schema).
        let table = TableBuilder::new(Some("app"), "orders")
            .lock_escalation(LockEscalation::Table)
            .build();
        let schema = SchemaBuilder::new(Some("app")).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_footer(&table);

        assert!(buffer.contents().contains("alter table app.orders set (lock_escalation = table)\nGO"));
    }
}
