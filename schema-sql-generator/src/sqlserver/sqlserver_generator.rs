use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::common::sql_string::escape_sql_literal;
use crate::sql_println;
use crate::sqlserver::sqlserver_function_generator::SqlServerFunctionGenerator;
use crate::sqlserver::sqlserver_index_generator::SqlServerIndexGenerator;
use crate::sqlserver::sqlserver_other_sql_generator::SqlServerOtherSqlGenerator;
use crate::sqlserver::sqlserver_procedure_generator::SqlServerProcedureGenerator;
use crate::sqlserver::sqlserver_relation_generator::SqlServerRelationGenerator;
use crate::sqlserver::sqlserver_table_generator::SqlServerTableGenerator;
use crate::sqlserver::sqlserver_trigger_generator::SqlServerTriggerGenerator;
use crate::sqlserver::sqlserver_view_generator::SqlServerViewGenerator;

pub struct SqlServerGenerator {
    sql_generator: DefaultSqlGenerator,
}

impl SqlServerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(SqlServerTableGenerator::new(context.clone())),
            Box::new(SqlServerRelationGenerator::new(context.clone())),
            Box::new(SqlServerIndexGenerator::new(context.clone())),
            Box::new(SqlServerFunctionGenerator::new(context.clone())),
            Box::new(SqlServerViewGenerator::new(context.clone())),
            Box::new(SqlServerProcedureGenerator::new(context.clone())),
            Box::new(SqlServerTriggerGenerator::new(context.clone())),
            Box::new(SqlServerOtherSqlGenerator::new(context.clone())),
        );

        Self {
            sql_generator,
        }
    }

    /// Emits an existence-guarded `create schema` for every non-default schema the model
    /// declares (C4). SQL Server requires `CREATE SCHEMA` to be the first statement in its
    /// batch, so it can't be guarded with a plain `if not exists (...) create schema ...` -
    /// the dynamic-SQL `exec(...)` form works around that restriction. `dbo` is skipped -
    /// every SQL Server database always has it.
    fn create_schemas(&self) {
        let context = self.context();
        let separator = context.settings().statement_separator().to_string();
        let database_model = context.settings().database_model();

        let mut schema_names: Vec<&str> = database_model
            .schemas()
            .iter()
            .filter_map(|schema| schema.schema_name())
            .filter(|name| !name.eq_ignore_ascii_case("dbo"))
            .collect();
        schema_names.sort_unstable();
        schema_names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        if schema_names.is_empty() {
            return;
        }

        context.with_writer(|writer| {
            for schema_name in schema_names {
                sql_println!(writer, "if not exists (select 1 from sys.schemas where name = '{}')", escape_sql_literal(schema_name));
                sql_println!(writer, "   exec('create schema {}'){}", schema_name, separator);
            }
            sql_println!(writer, "");
        });
    }
}

impl SqlGenerator for SqlServerGenerator {
    fn context(&self) -> &GeneratorContext {
        self.sql_generator.context()
    }

    // `generate`/`output_sql` are intentionally *not* overridden here: the trait's default
    // implementations call `self.output_sql()`/`self.output_header()` on `self`, which for a
    // `Box<dyn SqlGenerator>` is this type and correctly reaches the `output_header` override
    // below. Delegating them to `self.sql_generator.generate()`/`.output_sql()` (as this used
    // to do) would call those methods on the *concrete* `DefaultSqlGenerator` field instead -
    // the same static-dispatch trap as `SqliteTableGenerator`/`PostgresTableGenerator`
    // (see H1) - which would silently skip `create_schemas` below.

    fn output_header(&self) {
        self.create_schemas();
        self.sql_generator.output_header();
    }

    fn output_tables(&self) {
        self.sql_generator.output_tables();
    }

    fn output_relations(&self) {
        self.sql_generator.output_relations();
    }

    fn output_indexes(&self) {
        self.sql_generator.output_indexes();
    }

    fn output_triggers(&self) {
        self.sql_generator.output_triggers();
    }

    fn output_functions(&self) {
        self.sql_generator.output_functions();
    }

    fn output_views(&self) {
        self.sql_generator.output_views();
    }

    fn output_procedures(&self) {
        self.sql_generator.output_procedures();
    }

    fn output_other_sql_top(&self) {
        self.sql_generator.output_other_sql_top();
    }

    fn output_other_sql_bottom(&self) {
        self.sql_generator.output_other_sql_bottom();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_header_creates_non_default_schemas() {
        let default_schema = SchemaBuilder::new(None::<&str>).build();
        let app_schema = SchemaBuilder::new(Some("app")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![default_schema, app_schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(output.contains("if not exists (select 1 from sys.schemas where name = 'app')"));
        assert!(output.contains("exec('create schema app')\nGO"));
    }

    #[test]
    fn output_header_never_creates_the_dbo_schema() {
        // `dbo` always exists in SQL Server; guarding and creating it is unnecessary noise.
        let schema = SchemaBuilder::new(Some("dbo")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerGenerator::new(ctx);
        generator.output_header();

        assert!(!buffer.contents().contains("create schema"));
    }

    #[test]
    fn output_header_omits_schema_creation_when_only_default_schema_present() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerGenerator::new(ctx);
        generator.output_header();

        assert!(!buffer.contents().contains("create schema"));
    }

    #[test]
    fn generate_reaches_create_schemas_via_self_dispatch() {
        // Regression test: `generate()`/`output_sql()` must resolve `self.output_header()`
        // back to *this* type's override (not `DefaultSqlGenerator`'s no-op) even when called
        // through a `Box<dyn SqlGenerator>` - exactly how `GeneratorType::generate` invokes
        // it. Delegating `generate`/`output_sql` to the inner `sql_generator` field bypassed
        // this silently.
        let app_schema = SchemaBuilder::new(Some("app")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![app_schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator: Box<dyn SqlGenerator> = Box::new(SqlServerGenerator::new(ctx));
        generator.generate();

        assert!(buffer.contents().contains("exec('create schema app')"));
    }

    #[test]
    fn output_header_escapes_single_quote_in_schema_name() {
        let schema = SchemaBuilder::new(Some("o'brien")).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = SqlServerGenerator::new(ctx);
        generator.output_header();

        assert!(buffer.contents().contains("where name = 'o''brien'"));
    }
}
