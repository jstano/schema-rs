use std::collections::HashSet;
use std::io::Write;
use std::rc::Rc;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::builder::TableBuilder;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::table::Table;
use schema_model::model::types::{BooleanMode, DatabaseType, KeyType, RelationType};
use schema_model::naming::{foreign_key_name, index_name, primary_key_name, unique_key_name};
use schema_sql_generator::common::column_type_generator::ColumnTypeGenerator;
use schema_sql_generator::common::generator_context::{BufferSink, GeneratorContext};
use schema_sql_generator::common::sql_string::escape_sql_literal;
use schema_sql_generator::common::table_generator::TableGenerator;
use schema_sql_generator::common::trigger_generator::TriggerGenerator;
use schema_sql_generator::postgresql::postgres_column_type_generator::PostgresColumnTypeGenerator;
use schema_sql_generator::postgresql::postgres_table_generator::PostgresTableGenerator;
use schema_sql_generator::postgresql::postgres_trigger_generator::PostgresTriggerGenerator;
use schema_sql_generator::postgresql::postgres_util::to_snake_case;

use crate::check_constraint;
use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

#[derive(Default)]
pub struct PostgresqlMigrationGenerator {
    /// Target PostgreSQL major version (e.g. 17, 18) - threaded into every `GeneratorContext`
    /// this generator builds so version-gated column type SQL (e.g. the UUID default function)
    /// matches what `--postgresql-version` would produce on the create path. `0` means "unset".
    pub target_postgres_version: u32,
}

impl PostgresqlMigrationGenerator {
    pub fn new(target_postgres_version: u32) -> Self {
        Self { target_postgres_version }
    }
}

impl MigrationGenerator for PostgresqlMigrationGenerator {
    fn generate(
        &self,
        change_set: &ChangeSet,
        database_model: &DatabaseModel,
        writer: &mut dyn Write,
    ) -> Result<(), MigrationGeneratorError> {
        let context = GeneratorContext::for_model_with_target_version(
            Rc::new(database_model.clone()),
            DatabaseType::Postgresql,
            self.target_postgres_version,
        );
        let type_generator = PostgresColumnTypeGenerator::new(context.clone());
        let (trigger_context, trigger_buffer) = GeneratorContext::for_model_with_buffer_and_target_version(
            Rc::new(database_model.clone()),
            DatabaseType::Postgresql,
            self.target_postgres_version,
        );
        let trigger_generator = PostgresTriggerGenerator::new(trigger_context);
        let (table_context, table_buffer) = GeneratorContext::for_model_with_buffer_and_target_version(
            Rc::new(database_model.clone()),
            DatabaseType::Postgresql,
            self.target_postgres_version,
        );
        let table_generator = PostgresTableGenerator::new(table_context);
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();
        let mut triggers_regenerated: HashSet<String> = HashSet::new();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table } => {
                    write_add_table(&table_generator, &table_buffer, table, writer)?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(writer, "drop table if exists {};", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, column));
                    let default_value = default_sql(database_model, column);

                    if column.required() && default_value.is_none() {
                        write_not_null_column_with_backfill_guard(writer, table_name, column.name(), &type_sql)?;
                    } else {
                        let not_null = if column.required() { " not null" } else { "" };
                        let default = default_value.map(|d| format!(" default {}", d)).unwrap_or_default();
                        writeln!(
                            writer,
                            "alter table {} add column if not exists {}{}{}{};",
                            table_name,
                            column.name(),
                            type_sql,
                            not_null,
                            default
                        )?;
                        writeln!(writer)?;
                    }

                    // Enums are excluded: Postgres represents them as a native enum type
                    // (see `PostgresColumnTypeGenerator::enum_sql`), so the value list is
                    // already enforced by the type itself - unlike the other databases, which
                    // emulate enums with a plain string column plus a CHECK constraint (see
                    // `PostgresColumnConstraintGenerator`'s matching exclusion).
                    if column.column_type() != ColumnType::Enum
                        && let Some(check_sql) = check_constraint::check_constraint_sql(&context, column)
                    {
                        let name = check_constraint::constraint_name(table_name, column.name());
                        write_guarded_add_constraint(
                            writer,
                            table_name,
                            &name,
                            &format!("alter table {} add constraint {} {};", table_name, name, check_sql),
                        )?;
                    }
                }
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   alter table {} rename column {} to {};", table_name, column_name, candidate)?;
                        }
                    }
                    writeln!(writer, "alter table {} drop column if exists {};", table_name, column_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::ModifyColumn { table_name, old_column, new_column } => {
                    let (new_type, new_is_sequence) = alter_column_type_sql(new_column, &type_generator, &dummy_table);
                    let (old_type, _) = alter_column_type_sql(old_column, &type_generator, &dummy_table);
                    if new_type != old_type {
                        if new_is_sequence {
                            writeln!(
                                writer,
                                "-- NOTE: '{}' is a Sequence/LongSequence column - serial/bigserial is create-time-only sugar for {} + an owned sequence + a default, so ALTER COLUMN TYPE below only retargets the underlying type. If this column needs an owned sequence and default too, that requires manual DDL (create sequence ... owned by ..., then set default nextval(...)).",
                                new_column.name(),
                                new_type
                            )?;
                        }
                        // `using {column}::{type}` makes the cast explicit instead of relying
                        // on Postgres's implicit assignment cast, which doesn't exist for many
                        // pairs (e.g. varchar -> int: "column cannot be cast automatically").
                        // An explicit cast via `::` covers both that case and every case the
                        // implicit cast already handled, so it's always safe to add here.
                        writeln!(
                            writer,
                            "alter table {} alter column {} type {} using {}::{};",
                            table_name,
                            new_column.name(),
                            new_type,
                            new_column.name(),
                            new_type
                        )?;
                    }
                    if old_column.required() != new_column.required() {
                        if new_column.required() {
                            writeln!(
                                writer,
                                "alter table {} alter column {} set not null;",
                                table_name,
                                new_column.name()
                            )?;
                        } else {
                            writeln!(
                                writer,
                                "alter table {} alter column {} drop not null;",
                                table_name,
                                new_column.name()
                            )?;
                        }
                    }
                    if old_column.default_constraint() != new_column.default_constraint() {
                        if let Some(default) = default_sql(database_model, new_column) {
                            writeln!(
                                writer,
                                "alter table {} alter column {} set default {};",
                                table_name,
                                new_column.name(),
                                default
                            )?;
                        } else {
                            writeln!(
                                writer,
                                "alter table {} alter column {} drop default;",
                                table_name,
                                new_column.name()
                            )?;
                        }
                    }
                    writeln!(writer)?;
                }
                SchemaChange::AddKey { table_name, key, ordinal } => {
                    write_add_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::DropKey { table_name, key, ordinal } => {
                    write_drop_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::AddConstraint { table_name, constraint } if constraint.database_type() == DatabaseType::Postgresql => {
                    // `constraint.sql()` is already the full `check (...)` clause - same
                    // convention as `DefaultTableConstraintGenerator` on the create path
                    // (`constraint {name} {sql}`, no wrapper added) - so this must not
                    // re-wrap it, or it comes out as `check (check (...))`.
                    write_guarded_add_constraint(
                        writer,
                        table_name,
                        constraint.name(),
                        &format!(
                            "alter table {} add constraint {} {};",
                            table_name,
                            constraint.name(),
                            constraint.sql()
                        ),
                    )?;
                }
                SchemaChange::AddConstraint { .. } => {}
                SchemaChange::DropConstraint { table_name, constraint_name, database_type } if *database_type == DatabaseType::Postgresql => {
                    writeln!(
                        writer,
                        "alter table {} drop constraint if exists {};",
                        table_name, constraint_name
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::DropConstraint { .. } => {}
                SchemaChange::AddRelation { relation, ordinal } => {
                    write_add_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::DropRelation { relation, ordinal } => {
                    let fk_name = foreign_key_name(DatabaseType::Postgresql, relation.from_table_name(), *ordinal);
                    writeln!(
                        writer,
                        "alter table {} drop constraint if exists {};",
                        relation.from_table_name(),
                        fk_name
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { view }
                    if view.database_type().is_none() || view.database_type() == Some(DatabaseType::Postgresql) =>
                {
                    writeln!(writer, "create or replace view {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { .. } => {}
                SchemaChange::DropView { view_name, database_type }
                    if database_type.is_none() || *database_type == Some(DatabaseType::Postgresql) =>
                {
                    writeln!(writer, "drop view if exists {};", view_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { .. } => {}
                SchemaChange::AddEnumType { enum_type } => {
                    write_add_enum_type(writer, enum_type)?;
                }
                SchemaChange::DropEnumType { enum_type_name } => {
                    let name = to_snake_case(enum_type_name);
                    writeln!(writer, "drop type if exists {} cascade;", name)?;
                    writeln!(writer)?;
                }
                SchemaChange::ModifyEnumType { old_enum_type, new_enum_type } => {
                    write_modify_enum_type(writer, old_enum_type, new_enum_type)?;
                }
                SchemaChange::AddFunction { function } if function.database_type() == DatabaseType::Postgresql => {
                    writeln!(writer, "{};", function.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddFunction { .. } => {}
                SchemaChange::DropFunction { function_name, database_type } if *database_type == DatabaseType::Postgresql => {
                    writeln!(writer, "-- NOTE: add argument types below if '{}' is overloaded.", function_name)?;
                    writeln!(writer, "drop function if exists {};", function_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropFunction { .. } => {}
                SchemaChange::AddProcedure { procedure } if procedure.database_type() == DatabaseType::Postgresql => {
                    writeln!(writer, "{};", procedure.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddProcedure { .. } => {}
                SchemaChange::DropProcedure { procedure_name, database_type } if *database_type == DatabaseType::Postgresql => {
                    writeln!(writer, "-- NOTE: add argument types below if '{}' is overloaded.", procedure_name)?;
                    writeln!(writer, "drop procedure if exists {};", procedure_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropProcedure { .. } => {}
                SchemaChange::AddOtherSql { other_sql } if other_sql.database_type() == DatabaseType::Postgresql => {
                    writeln!(writer, "{};", other_sql.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddOtherSql { .. } => {}
                SchemaChange::DropOtherSql { other_sql } if other_sql.database_type() == DatabaseType::Postgresql => {
                    writeln!(writer, "-- TODO: other_sql entry removed, review manually: {}", other_sql.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropOtherSql { .. } => {}
                SchemaChange::AddTrigger { table_name, trigger } if trigger.database_type() == DatabaseType::Postgresql => {
                    regenerate_triggers(&trigger_generator, &trigger_buffer, database_model, table_name, &mut triggers_regenerated, &mut *writer)?;
                }
                SchemaChange::AddTrigger { .. } => {}
                SchemaChange::DropTrigger { table_name, trigger } if trigger.database_type() == DatabaseType::Postgresql => {
                    regenerate_triggers(&trigger_generator, &trigger_buffer, database_model, table_name, &mut triggers_regenerated, &mut *writer)?;
                }
                SchemaChange::DropTrigger { .. } => {}
                SchemaChange::AddInitialData { initial_data, .. }
                    if initial_data.database_type().is_none() || initial_data.database_type() == Some(DatabaseType::Postgresql) =>
                {
                    writeln!(writer, "{};", initial_data.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddInitialData { .. } => {}
                SchemaChange::DropInitialData { initial_data, .. }
                    if initial_data.database_type().is_none() || initial_data.database_type() == Some(DatabaseType::Postgresql) =>
                {
                    writeln!(writer, "-- TODO: initial_data entry removed, review manually: {}", initial_data.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropInitialData { .. } => {}
            }
        }

        Ok(())
    }
}

/// Renders a brand-new table's `CREATE TABLE` by routing it through
/// `schema-sql-generator`'s own `PostgresTableGenerator` (C7) - the same column/key/constraint
/// type mapping the full-schema generator uses - rather than a second, divergent one. Only
/// `output_table_definition`/`output_indexes`/`output_initial_data` are called: `output_table`
/// itself would also call `output_table_drop` (an unconditional `drop table ... cascade`, wrong
/// here) and `output_table_header` (a bare `create table {name}`, not the idempotent
/// `if not exists` form a migration needs), so the header/footer are written directly instead.
/// Relations and triggers are deliberately not included here - they're always a separate
/// generation pass, even for an existing table - see `diff_add_relations`/`diff_add_triggers`
/// in `schema-diff`, which still emit `AddRelation`/`AddTrigger` for a brand-new table.
fn write_add_table(
    table_generator: &PostgresTableGenerator,
    table_buffer: &BufferSink,
    table: &Table,
    writer: &mut dyn Write,
) -> Result<(), MigrationGeneratorError> {
    let qualified_name = table.fully_qualified_table_name(DatabaseType::Postgresql);

    table_generator.output_table_definition(table);
    let body = table_buffer.take();
    writeln!(writer, "create table if not exists {} (", qualified_name)?;
    write!(writer, "{}", body)?;
    writeln!(writer, ");")?;
    writeln!(writer)?;

    table_generator.output_indexes(table);
    write!(writer, "{}", table_buffer.take())?;
    table_generator.output_initial_data(table);
    write!(writer, "{}", table_buffer.take())?;
    Ok(())
}

/// The Postgres full-schema generator writes trigger SQL through `GeneratorContext`'s own
/// internal buffer (`SqlWriter`), not a `dyn Write` directly - so regenerating a table's
/// triggers here means asking the generator to render into a dedicated `BufferSink`
/// (`trigger_buffer`), then draining and copying that text into `writer` right away, so it
/// lands in the right place relative to the rest of the migration's output. `regenerated`
/// prevents re-rendering the same table twice when both an `AddTrigger` and a `DropTrigger`
/// land on it in one run.
fn regenerate_triggers(
    trigger_generator: &PostgresTriggerGenerator,
    trigger_buffer: &schema_sql_generator::common::generator_context::BufferSink,
    database_model: &DatabaseModel,
    table_name: &str,
    regenerated: &mut HashSet<String>,
    writer: &mut dyn Write,
) -> Result<(), MigrationGeneratorError> {
    if !regenerated.insert(table_name.to_string()) {
        return Ok(());
    }
    if let Some(table) = database_model.all_tables().into_iter().find(|t| t.name().eq_ignore_ascii_case(table_name)) {
        trigger_generator.output_triggers_for_table(table);
        write!(writer, "{}", trigger_buffer.take())?;
    }
    Ok(())
}

/// Guarded so re-running the migration after the type already exists is a no-op, matching
/// `write_guarded_add_constraint`'s `pg_constraint` pattern but against `pg_type`.
fn write_add_enum_type(writer: &mut dyn Write, enum_type: &schema_model::model::enum_type::EnumType) -> Result<(), MigrationGeneratorError> {
    let name = to_snake_case(enum_type.name());
    let values = enum_type
        .values()
        .iter()
        .map(|v| format!("'{}'", escape_sql_literal(v.code())))
        .collect::<Vec<_>>()
        .join(",");

    writeln!(writer, "do $$")?;
    writeln!(writer, "begin")?;
    writeln!(writer, "  if not exists (select 1 from pg_type where typname = '{}') then", name)?;
    writeln!(writer, "    create type {} as enum ({});", name, values)?;
    writeln!(writer, "  end if;")?;
    writeln!(writer, "end $$;")?;
    writeln!(writer)?;
    Ok(())
}

/// Postgres has no `ALTER TYPE ... DROP VALUE`, so a removed value can't be enforced
/// automatically - existing rows may still hold it. Rather than a comment a developer could
/// miss, a removed value gets a statement that unconditionally fails the migration (`raise
/// exception`, always executed, not just when data happens to violate anything) until the
/// developer either confirms no rows use the value (deletes the block) or adds `update`
/// statements above it to migrate them first.
fn write_modify_enum_type(
    writer: &mut dyn Write,
    old_enum_type: &schema_model::model::enum_type::EnumType,
    new_enum_type: &schema_model::model::enum_type::EnumType,
) -> Result<(), MigrationGeneratorError> {
    let name = to_snake_case(new_enum_type.name());

    for value in new_enum_type.values() {
        if !old_enum_type.values().iter().any(|v| v.name() == value.name()) {
            writeln!(
                writer,
                "alter type {} add value if not exists '{}';",
                name,
                escape_sql_literal(value.code())
            )?;
        }
    }
    writeln!(writer)?;

    for value in old_enum_type.values() {
        if !new_enum_type.values().iter().any(|v| v.name() == value.name()) {
            writeln!(
                writer,
                "-- WARNING: enum value '{}' was removed from '{}'. Postgres has no ALTER TYPE ... DROP VALUE,",
                value.name(),
                name
            )?;
            writeln!(writer, "-- so this migration cannot enforce the new value list automatically. Before it can run:")?;
            writeln!(
                writer,
                "--   1. If rows still use '{}', add UPDATE statements above this line to migrate them to a valid value, then delete the block below, OR",
                value.name()
            )?;
            writeln!(
                writer,
                "--   2. If no rows use '{}', delete the block below to confirm that.",
                value.name()
            )?;
            writeln!(writer, "do $$")?;
            writeln!(writer, "begin")?;
            writeln!(
                writer,
                "  raise exception 'Migration halted: enum value ''{}'' was removed from {} - see comment above.';",
                escape_sql_literal(value.name()),
                name
            )?;
            writeln!(writer, "end $$;")?;
            writeln!(writer)?;
        }
    }

    Ok(())
}

/// A required column with no `default` has no value to backfill existing rows with -
/// `alter table ... add column ... not null` would just fail with `column "x" contains null
/// values` the moment the table has any rows (same reasoning as `write_modify_enum_type`'s
/// removed-value guard: an unconditional halt every time, not a silent pass-through when the
/// table happens to be empty). Adds the column nullable, forces an explicit developer decision
/// (backfill it or confirm the table is empty), then tightens it to `NOT NULL` - reachable only
/// once the guard above it has been deleted.
fn write_not_null_column_with_backfill_guard(
    writer: &mut dyn Write,
    table_name: &str,
    column_name: &str,
    type_sql: &str,
) -> Result<(), MigrationGeneratorError> {
    writeln!(writer, "alter table {} add column if not exists {}{};", table_name, column_name, type_sql)?;
    writeln!(writer)?;
    writeln!(
        writer,
        "-- WARNING: '{}' is being made NOT NULL with no default - existing rows have no value to backfill it with automatically.",
        column_name
    )?;
    writeln!(
        writer,
        "--   1. If '{}' has rows, add UPDATE statements above this line to give '{}' a value, then delete the block below, OR",
        table_name, column_name
    )?;
    writeln!(writer, "--   2. If '{}' is empty, delete the block below to confirm that.", table_name)?;
    writeln!(writer, "do $$")?;
    writeln!(writer, "begin")?;
    writeln!(
        writer,
        "  raise exception 'Migration halted: {} has no value for {} - see comment above.';",
        table_name, column_name
    )?;
    writeln!(writer, "end $$;")?;
    writeln!(writer)?;
    writeln!(writer, "alter table {} alter column {} set not null;", table_name, column_name)?;
    writeln!(writer)?;
    Ok(())
}

/// `PostgresColumnTypeGenerator::sequence_sql`/`long_sequence_sql` emit `serial`/`bigserial`,
/// which are create-time-only sugar (`integer`/`bigint` + an owned sequence + a `nextval(...)`
/// default) - not real type names, so `type "serial" does not exist` on `ALTER COLUMN ... TYPE`.
/// Returns the real underlying type to use there instead, plus whether a note about the
/// stripped-off sequence/default should be emitted (mirrors the SQL Server `is_identity`
/// handling in `sqlserver/mod.rs`, which hits the same problem with `identity(...)`).
fn alter_column_type_sql(column: &Column, type_generator: &PostgresColumnTypeGenerator, dummy_table: &schema_model::model::table::Table) -> (String, bool) {
    match column.column_type() {
        ColumnType::Sequence => ("integer".to_string(), true),
        ColumnType::LongSequence => ("bigint".to_string(), true),
        _ => (type_generator.column_type_sql(dummy_table, column), false),
    }
}

/// The `default` XML attribute is free-text SQL for every column type except `Boolean`,
/// which is authored as `true`/`false`/`yes`/`no`/etc and must be rendered as whatever
/// literal `boolean_mode` expects (e.g. `'Yes'`/`'No'` for `YesNo`) - see
/// `DefaultColumnGenerator::convert_boolean_default_constraint` in schema-sql-generator.
fn default_sql(database_model: &DatabaseModel, column: &Column) -> Option<String> {
    let raw = column.default_constraint()?;
    if column.column_type() == ColumnType::Boolean {
        return boolean_default_literal(database_model.boolean_mode(), raw);
    }
    Some(raw.to_string())
}

fn boolean_default_literal(boolean_mode: BooleanMode, raw: &str) -> Option<String> {
    let value = match schema_model::model::column::parse_boolean_default(raw) {
        Ok(Some(value)) => value,
        Ok(None) => return None,
        Err(()) => false,
    };
    Some(
        match boolean_mode {
            BooleanMode::Native => if value { "true" } else { "false" },
            BooleanMode::YesNo => if value { "'Yes'" } else { "'No'" },
            BooleanMode::YN => if value { "'Y'" } else { "'N'" },
        }
        .to_string(),
    )
}

/// Postgres has no `ADD CONSTRAINT IF NOT EXISTS` (for CHECK, PRIMARY KEY, or FOREIGN KEY
/// constraints), so idempotency has to be expressed as a `pg_constraint` existence check
/// wrapped in a `DO` block instead of a plain clause.
fn write_guarded_add_constraint(
    writer: &mut dyn Write,
    table_name: &str,
    constraint_name: &str,
    add_constraint_sql: &str,
) -> Result<(), MigrationGeneratorError> {
    writeln!(writer, "do $$")?;
    writeln!(writer, "begin")?;
    writeln!(
        writer,
        "  if not exists (select 1 from pg_constraint where conname = '{}' and conrelid = '{}'::regclass) then",
        constraint_name, table_name
    )?;
    writeln!(writer, "    {}", add_constraint_sql)?;
    writeln!(writer, "  end if;")?;
    writeln!(writer, "end $$;")?;
    writeln!(writer)?;
    Ok(())
}

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let cols: String = key.columns().iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
    match key.key_type() {
        KeyType::Primary => {
            let constraint_name = primary_key_name(DatabaseType::Postgresql, table_name);
            write_guarded_add_constraint(
                writer,
                table_name,
                &constraint_name,
                &format!("alter table {} add constraint {} primary key ({});", table_name, constraint_name, cols),
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Postgresql, table_name, ordinal);
            write_guarded_add_constraint(
                writer,
                table_name,
                &constraint_name,
                &format!("alter table {} add constraint {} unique ({});", table_name, constraint_name, cols),
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
            let unique = if key.is_unique() { "unique " } else { "" };
            let where_clause = key.filter().map(|f| format!(" where {}", f)).unwrap_or_default();
            writeln!(
                writer,
                "create {}index if not exists {} on {} ({}){};",
                unique, idx_name, table_name, cols, where_clause
            )?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

fn write_drop_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    match key.key_type() {
        KeyType::Primary => {
            let constraint_name = primary_key_name(DatabaseType::Postgresql, table_name);
            writeln!(
                writer,
                "alter table {} drop constraint if exists {};",
                table_name, constraint_name
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(
                writer,
                "alter table {} drop constraint if exists {};",
                table_name, constraint_name
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(writer, "drop index if exists {};", idx_name)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::Postgresql, relation.from_table_name(), ordinal);
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " on delete cascade",
        RelationType::SetNull => " on delete set null",
        RelationType::DoNothing => " on delete restrict",
        RelationType::Enforce => "",
    };
    let from_columns = relation.column_pairs().iter().map(|(from, _)| from.as_str()).collect::<Vec<_>>().join(", ");
    let to_columns = relation.column_pairs().iter().map(|(_, to)| to.as_str()).collect::<Vec<_>>().join(", ");
    write_guarded_add_constraint(
        writer,
        relation.from_table_name(),
        &fk_name,
        &format!(
            "alter table {} add constraint {} foreign key ({}) references {}({}){};",
            relation.from_table_name(),
            fk_name,
            from_columns,
            relation.to_table_name(),
            to_columns,
            on_delete
        ),
    )?;
    Ok(())
}
