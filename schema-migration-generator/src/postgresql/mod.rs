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
use schema_model::model::types::{BooleanMode, DatabaseType, KeyType, RelationType};
use schema_model::naming::{foreign_key_name, index_name, primary_key_name, unique_key_name};
use schema_sql_generator::common::column_type_generator::ColumnTypeGenerator;
use schema_sql_generator::common::generator_context::GeneratorContext;
use schema_sql_generator::common::sql_string::escape_sql_literal;
use schema_sql_generator::common::trigger_generator::TriggerGenerator;
use schema_sql_generator::postgresql::postgres_column_type_generator::PostgresColumnTypeGenerator;
use schema_sql_generator::postgresql::postgres_trigger_generator::PostgresTriggerGenerator;
use schema_sql_generator::postgresql::postgres_util::to_snake_case;

use crate::check_constraint;
use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

pub struct PostgresqlMigrationGenerator;

impl MigrationGenerator for PostgresqlMigrationGenerator {
    fn generate(
        &self,
        change_set: &ChangeSet,
        database_model: &DatabaseModel,
        writer: &mut dyn Write,
    ) -> Result<(), MigrationGeneratorError> {
        let context = GeneratorContext::for_model(Rc::new(database_model.clone()), DatabaseType::Postgresql);
        let type_generator = PostgresColumnTypeGenerator::new(context.clone());
        let (trigger_context, trigger_buffer) =
            GeneratorContext::for_model_with_buffer(Rc::new(database_model.clone()), DatabaseType::Postgresql);
        let trigger_generator = PostgresTriggerGenerator::new(trigger_context);
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();
        let mut triggers_regenerated: HashSet<String> = HashSet::new();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    writeln!(writer, "create table if not exists {} ();", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(writer, "drop table if exists {};", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameTable { old_name, new_name } => {
                    writeln!(writer, "alter table if exists {} rename to {};", old_name, new_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, column));
                    let not_null = if column.required() { " not null" } else { "" };
                    let default = default_sql(database_model, column)
                        .map(|d| format!(" default {}", d))
                        .unwrap_or_default();
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
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    writeln!(writer, "do $$")?;
                    writeln!(writer, "begin")?;
                    writeln!(
                        writer,
                        "  if exists (select 1 from information_schema.columns where table_name = '{}' and column_name = '{}') then",
                        table_name, old_name
                    )?;
                    writeln!(
                        writer,
                        "    alter table {} rename column {} to {};",
                        table_name, old_name, new_name
                    )?;
                    writeln!(writer, "  end if;")?;
                    writeln!(writer, "end $$;")?;
                    writeln!(writer)?;
                }
                SchemaChange::ModifyColumn { table_name, old_column, new_column } => {
                    let new_type = format!(" {}", type_generator.column_type_sql(&dummy_table, new_column));
                    let old_type = format!(" {}", type_generator.column_type_sql(&dummy_table, old_column));
                    if new_type != old_type {
                        writeln!(
                            writer,
                            "alter table {} alter column {} type {};",
                            table_name,
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
                SchemaChange::AddConstraint { table_name, constraint } => {
                    write_guarded_add_constraint(
                        writer,
                        table_name,
                        constraint.name(),
                        &format!(
                            "alter table {} add constraint {} check ({});",
                            table_name,
                            constraint.name(),
                            constraint.sql()
                        ),
                    )?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    writeln!(
                        writer,
                        "alter table {} drop constraint if exists {};",
                        table_name, constraint_name
                    )?;
                    writeln!(writer)?;
                }
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
                SchemaChange::AddView { view } => {
                    writeln!(writer, "create or replace view {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { view_name } => {
                    writeln!(writer, "drop view if exists {};", view_name)?;
                    writeln!(writer)?;
                }
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
            writeln!(
                writer,
                "create unique index if not exists {} on {} ({});",
                constraint_name, table_name, cols
            )?;
            writeln!(writer)?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(
                writer,
                "create index if not exists {} on {} ({});",
                idx_name, table_name, cols
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
            writeln!(writer, "drop index if exists {};", constraint_name)?;
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
