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

        // All drops up front, in reverse-dependency order, rather than interleaved with
        // each table's own create (H17): SQL Server has no `cascade` on `drop table`, so
        // dropping in declaration order can fail on the first table that's still an FK
        // target of a later one. See `tables_in_drop_order`.
        for table in crate::common::table_generator::tables_in_drop_order(database_model) {
            self.output_table_drop(table);
        }

        for schema in database_model.schemas() {
            for table in schema.tables() {
                self.output_table_header(table);
                self.output_table_definition(table);
                self.output_table_footer(table);
                self.output_indexes(table);
                self.output_initial_data(table);
            }
        }
    }

    fn output_table(&self, table: &Table) {
        self.output_table_drop(table);
        self.output_table_header(table);
        self.output_table_definition(table);
        self.output_table_footer(table);
        self.output_indexes(table);
        self.output_initial_data(table);
    }

    fn output_table_drop(&self, table: &Table) {
        // The legacy Java codegen tool's SQL Server output uses an existence-checked drop
        // (`if object_id(...) is not null`) rather than the `drop table if exists ...;` form
        // the other dialects share, so this is a full override rather than a delegation to
        // `DefaultTableGenerator::output_table_drop`.
        // The guard is schema-qualified (H15): querying `object_id('{schema}.{table}', 'U')`
        // rather than `dbo.sysobjects` on name alone, since the latter ignores schema and
        // would drop/skip the wrong object when the same table name exists in two schemas.
        let database_type = self.context.settings().database_type();
        let separator = self.context.settings().statement_separator().to_string();
        let fully_qualified_table_name = table.fully_qualified_table_name(database_type);
        let table_name = table.name();

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {} */", table_name);
            sql_println!(writer, "if object_id('{}', 'U') is not null", escape_sql_literal(&fully_qualified_table_name));
            sql_println!(writer, "drop table {}{}", fully_qualified_table_name, separator);
            sql_println!(writer, "");
        });
    }

    fn output_table_header(&self, table: &Table) {
        let fully_qualified_table_name = table.fully_qualified_table_name(self.context.settings().database_type());

        self.context.with_writer(|writer| {
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
        generator.output_table_drop(&table);
        generator.output_table_header(&table);
        generator.output_table_definition(&table);
        generator.output_table_footer(&table);

        let output = buffer.contents();
        assert!(output.contains("/* users */"));
        assert!(output.contains("if object_id('dbo.users', 'U') is not null"));
        assert!(output.contains("drop table dbo.users\nGO"));
        assert!(output.contains("create table dbo.users"));
        assert!(output.contains("id integer identity(1,1)"));
        assert!(output.contains("name nvarchar(50)"));
    }

    #[test]
    fn output_table_header_escapes_single_quote_in_table_name() {
        // Regression test: an unescaped embedded quote would break the generated
        // object_id existence-check SQL string literal.
        let table = TableBuilder::new(None::<&str>, "o'brien").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_table_drop(&table);

        let output = buffer.contents();
        assert!(output.contains("if object_id('dbo.o''brien', 'U') is not null"));
    }

    #[test]
    fn output_tables_drops_child_before_parent_and_before_any_create() {
        // Regression test for H17: dropping in declaration order (parent first, since
        // it must be declared before the child that references it) fails on a rerun
        // against a populated database - SQL Server has no `cascade` on `drop table`,
        // so `drop table parent` errors while `child`'s FK constraint still references
        // it. All drops must be emitted up front, in reverse-dependency order (child
        // before parent), ahead of every create.
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(schema_model::model::relation::Relation::new(
                "parent", "id", "child", "parent_id", schema_model::model::types::RelationType::Cascade, false,
            ))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerTableGenerator::new(ctx);
        generator.output_tables();

        let output = buffer.contents();
        let drop_child = output.find("drop table dbo.child").unwrap();
        let drop_parent = output.find("drop table dbo.parent").unwrap();
        let create_child = output.find("create table dbo.child").unwrap();
        let create_parent = output.find("create table dbo.parent").unwrap();

        assert!(drop_child < drop_parent, "child must be dropped before the parent it references");
        assert!(drop_parent < create_child, "every drop must precede every create");
        assert!(drop_parent < create_parent, "every drop must precede every create");
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
        // other statement for the table (create/drop/object_id check) used the fully
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
