use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::postgresql::postgres_function_generator::PostgresFunctionGenerator;
use crate::postgresql::postgres_index_generator::PostgresIndexGenerator;
use crate::postgresql::postgres_other_sql_generator::PostgresOtherSqlGenerator;
use crate::postgresql::postgres_procedure_generator::PostgresProcedureGenerator;
use crate::postgresql::postgres_relation_generator::PostgresRelationGenerator;
use crate::postgresql::postgres_table_generator::PostgresTableGenerator;
use crate::postgresql::postgres_trigger_generator::PostgresTriggerGenerator;
use crate::postgresql::postgres_util::to_snake_case;
use crate::postgresql::postgres_view_generator::PostgresViewGenerator;
use crate::sql_println;

pub struct PostgresGenerator {
    context: GeneratorContext,
    sql_generator: DefaultSqlGenerator,
}

impl PostgresGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(PostgresTableGenerator::new(context.clone())),
            Box::new(PostgresRelationGenerator::new(context.clone())),
            Box::new(PostgresIndexGenerator::new(context.clone())),
            Box::new(PostgresFunctionGenerator::new(context.clone())),
            Box::new(PostgresViewGenerator::new(context.clone())),
            Box::new(PostgresProcedureGenerator::new(context.clone())),
            Box::new(PostgresTriggerGenerator::new(context.clone())),
            Box::new(PostgresOtherSqlGenerator::new(context.clone())),
        );

        Self {
            context : context.clone(),
            sql_generator,
        }
    }

    fn create_uuid_generator_function(&self) {
        let separator = self.context.settings().statement_separator().to_string();

        self.context.with_writer(|writer| {
            sql_println!(writer, "create or replace function generate_uuid() returns uuid language plpgsql volatile parallel unsafe as $$");
            sql_println!(writer, "declare");
            sql_println!(writer, "   -- The current UNIX timestamp in milliseconds");
            sql_println!(writer, "   unix_time_ms CONSTANT bigint NOT NULL DEFAULT (extract(epoch FROM clock_timestamp()) * 1000)::bigint;");
            sql_println!(writer, "");
            sql_println!(writer, "   -- The buffer used to create the UUID: the low 6 bytes (48 bits) of the timestamp, followed by 10 random bytes");
            sql_println!(writer, "   buffer bytea not null default substring(int8send(unix_time_ms) from 3) || gen_random_bytes(10);");
            sql_println!(writer, "begin");
            sql_println!(writer, "   -- Set the version nibble of byte 6 to 0111 (UUID v7), keeping the last 4 bits unchanged");
            sql_println!(writer, "   buffer = set_byte(buffer, 6, (get_byte(buffer, 6) & 15) | 112);");
            sql_println!(writer, "");
            sql_println!(writer, "   -- Set the top 2 bits of byte 8 to 10 (the UUID variant specified in RFC 4122), keeping the last 6 bits unchanged");
            sql_println!(writer,
                "   buffer = set_byte(buffer, 8, (get_byte(buffer, 8) & 63) | 128);",
            );
            sql_println!(writer, "");
            sql_println!(writer, "   return encode(buffer, 'hex')::uuid;");
            sql_println!(writer, "end");
            sql_println!(writer, "$${}", separator);
            sql_println!(writer, "");
        });
    }

    fn create_extensions(&self) {
        let separator = self.context.settings().statement_separator().to_string();
        let check_user = match self.context.settings().extension_check_user() {
            Some(user) => format!("'{}'", user),
            None => "CURRENT_USER".to_string(),
        };

        self.context.with_writer(|writer| {
            sql_println!(writer, "do $$");
            sql_println!(writer, "begin");
            sql_println!(writer, "   if (select usesuper from pg_user where usename = {}) then", check_user);
            sql_println!(writer, "      create extension if not exists \"citext\";");
            sql_println!(writer, "      create extension if not exists \"btree_gist\";");
            sql_println!(writer, "   else");
            sql_println!(writer, "      raise notice 'Could not create extensions, user % does not have permission.', current_user;");
            sql_println!(writer, "   end if;");
            sql_println!(writer, "end;");
            sql_println!(writer, "$${}", separator);
            sql_println!(writer, "");
        });
    }

    fn create_enum_types(&self) {
        let separator = self.context.settings().statement_separator().to_string();
        let database_model = self.context.settings().database_model();

        for schema in database_model.schemas() {
            for enum_type in schema.enum_types() {
                let enum_type_name = to_snake_case(enum_type.name());
                let values = enum_type
                    .values()
                    .iter()
                    .map(|v| format!("'{}'", v.code()))
                    .collect::<Vec<_>>()
                    .join(",");

                self.context.with_writer(|writer| {
                    sql_println!(writer, "drop type if exists {} cascade{}", enum_type_name, separator);
                    sql_println!(writer, "create type {} as enum ({}){}", enum_type_name, values, separator);
                    sql_println!(writer, "");
                });
            }
        }
    }
}

impl SqlGenerator for PostgresGenerator {
    fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn output_header(&self) {
        if self.context.settings().target_postgres_version() < 18 {
            self.create_uuid_generator_function();
        }
        if self.context.settings().emit_postgres_extensions() {
            self.create_extensions();
        }
        self.create_enum_types();
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
    use crate::common::generate_options::GenerateOptions;
    use crate::common::print_writer::PrintWriter;
    use crate::common::sql_generator_settings::SqlGeneratorSettings;
    use crate::common::sql_writer::SqlWriter;
    use crate::common::test_support::SharedBuffer;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::enum_type::{EnumType, EnumValue};
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_context_with_version(model: DatabaseModel, target_postgres_version: u32) -> (GeneratorContext, SharedBuffer) {
        make_context(model, target_postgres_version, true)
    }

    fn make_context(model: DatabaseModel, target_postgres_version: u32, emit_postgres_extensions: bool) -> (GeneratorContext, SharedBuffer) {
        let buffer = SharedBuffer::new();
        let mut options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
        );
        options.target_postgres_version = target_postgres_version;
        options.emit_postgres_extensions = emit_postgres_extensions;
        let settings = SqlGeneratorSettings::new(DatabaseType::Postgresql, &options);
        let writer = SqlWriter::new(options.writer.clone());
        (GeneratorContext::new(settings, writer), buffer)
    }

    #[test]
    fn output_header_includes_uuid_function_before_postgres_18() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_version(model, 17);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(output.contains("create or replace function generate_uuid()"));
    }

    #[test]
    fn output_header_omits_uuid_function_on_postgres_18_and_later() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_version(model, 18);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(!output.contains("create or replace function generate_uuid()"));
    }

    #[test]
    fn output_header_renders_enum_types_when_present() {
        let enum_type = EnumType::new(
            "status_type",
            vec![
                EnumValue::new("ACTIVE", Some("A".to_string())),
                EnumValue::new("INACTIVE", Some("I".to_string())),
            ],
        );
        let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_version(model, 18);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(output.contains("create type status_type as enum ('A','I')"));
    }

    #[test]
    fn output_header_omits_enum_ddl_when_no_enums_defined() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_version(model, 18);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(!output.contains("create type"));
    }

    #[test]
    fn output_header_includes_extensions_block_by_default() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_version(model, 18);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(output.contains("create extension if not exists \"citext\""));
        assert!(output.contains("create extension if not exists \"btree_gist\""));
    }

    #[test]
    fn output_header_omits_extensions_block_when_disabled() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, 18, false);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(!output.contains("create extension"));
    }

    #[test]
    fn output_header_checks_current_user_by_default() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_version(model, 18);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(output.contains("where usename = CURRENT_USER"));
    }

    #[test]
    fn output_header_checks_configured_extension_user() {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let buffer = SharedBuffer::new();
        let mut options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
        );
        options.target_postgres_version = 18;
        options.extension_check_user = Some("schema_admin".to_string());
        let settings = SqlGeneratorSettings::new(DatabaseType::Postgresql, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let ctx = GeneratorContext::new(settings, writer);

        let generator = PostgresGenerator::new(ctx);
        generator.output_header();

        let output = buffer.contents();
        assert!(output.contains("where usename = 'schema_admin'"));
        assert!(!output.contains("CURRENT_USER)"));
    }
}
