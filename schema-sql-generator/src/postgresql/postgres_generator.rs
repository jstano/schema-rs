use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::postgresql::postgres_function_generator::PostgresFunctionGenerator;
use crate::postgresql::postgres_index_generator::PostgresIndexGenerator;
use crate::postgresql::postgres_other_sql_generator::PostgresOtherSqlGenerator;
use crate::postgresql::postgres_procedure_generator::PostgresProcedureGenerator;
use crate::postgresql::postgres_relation_generator::PostgresRelationGenerator;
use crate::postgresql::postgres_table_generator::PostgresTableGenerator;
use crate::postgresql::postgres_trigger_generator::PostgresTriggerGenerator;
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

    pub fn output_header(&mut self) {
        self.create_uuid_generator_function();
        self.create_extensions();
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
            sql_println!(writer, "do $createextensions$");
            sql_println!(writer, "begin");
            sql_println!(writer, "   if (select usesuper from pg_user where usename = CURRENT_USER) then");
            sql_println!(writer, "      create extension if not exists \"uuid-ossp\";");
            sql_println!(writer, "      create extension if not exists \"citext\";");
            sql_println!(writer, "      create extension if not exists \"btree_gist\";");
            sql_println!(writer, "   else");
            sql_println!(writer, "      raise notice 'User % is not a superuser, could not create uuid-ossp or citext extensions.', current_user;");
            sql_println!(writer, "   end if;");
            sql_println!(writer, "end;");
            sql_println!(writer, "$createextensions${}", separator);
            sql_println!(writer, "");
        });
    }
}

impl SqlGenerator for PostgresGenerator {
    fn generate(&self) {
        self.sql_generator.generate()
    }

    fn output_sql(&self) {
        self.sql_generator.output_sql();
    }

    fn output_header(&self) {
        self.create_uuid_generator_function();
        self.create_extensions();
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
