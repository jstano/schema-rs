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
            sql_println!(writer, "create or replace function generate_uuid() returns uuid language plpgsql parallel safe as $$");
            sql_println!(writer, "declare");
            sql_println!(writer, "   -- The current UNIX timestamp in milliseconds");
            sql_println!(writer, "   unix_time_ms CONSTANT bytea NOT NULL DEFAULT substring(int8send((extract(epoch FROM clock_timestamp()) * 1000)::bigint) from 3);");
            sql_println!(writer, "");
            sql_println!(writer, "   -- The buffer used to create the UUID, starting with the UNIX timestamp and followed by random bytes");
            sql_println!(writer, "   buffer bytea not null default unix_time_ms || gen_random_bytes(10);");
            sql_println!(writer, "begin");
            sql_println!(writer, "   -- Set most significant 4 bits of 7th byte to 7 (for UUID v7), keeping the last 4 bits unchanged");
            sql_println!(writer, "   buffer = set_byte(buffer, 6, (b'0111' || get_byte(buffer, 6)::bit(4))::bit(8)::int);");
            sql_println!(writer, "");
            sql_println!(writer, "   -- Set most significant 2 bits of 9th byte to 2 (the UUID variant specified in RFC 4122), keeping the last 6 bits unchanged");
            sql_println!(writer,
                "   buffer = set_byte(buffer, 8, (b'10' || get_byte(buffer, 8)::bit(6))::bit(8)::int);",
            );
            sql_println!(writer, "");
            sql_println!(writer, "   return encode(buffer, 'hex');");
            sql_println!(writer, "end");
            sql_println!(writer, "$${}", separator);
            sql_println!(writer, "");
        });
    }

    fn create_extensions(&self) {
        let separator = self.context.settings().statement_separator().to_string();

        self.context.with_writer(|writer| {
            sql_println!(writer, "do $$");
            sql_println!(writer, "begin");
            sql_println!(writer, "   if (select usesuper from pg_user where usename = CURRENT_USER) then");
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

        let mut has_enums = false;
        let mut enum_ddl = String::new();

        for schema in database_model.schemas() {
            for enum_type in schema.enum_types() {
                has_enums = true;
                let schema_prefix = if let Some(schema_name) = schema.schema_name() {
                    format!("{}.", schema_name.to_lowercase())
                } else {
                    String::new()
                };

                let enum_type_name = to_snake_case(enum_type.name());
                let values = enum_type
                    .values()
                    .iter()
                    .map(|v| format!("'{}'", v.code()))
                    .collect::<Vec<_>>()
                    .join(", ");

                enum_ddl.push_str(&format!("drop type if exists {}{} cascade{}",
                    schema_prefix,
                    enum_type_name,
                    separator
                ));
                enum_ddl.push('\n');
                enum_ddl.push_str(&format!("create type {}{} as enum ({}){}",
                    schema_prefix,
                    enum_type_name,
                    values,
                    separator
                ));
                enum_ddl.push('\n');
            }
        }

        if has_enums {
            self.context.with_writer(|writer| {
                writer.println(&enum_ddl);
                sql_println!(writer, "");
            });
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
        self.create_extensions();
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
        let buffer = SharedBuffer::new();
        let mut options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
        );
        options.target_postgres_version = target_postgres_version;
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
        assert!(output.contains("create type status_type as enum ('A', 'I')"));
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
}
