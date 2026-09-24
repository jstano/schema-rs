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
use schema_sql_generator::common::trigger_generator::TriggerGenerator;
use schema_sql_generator::sqlserver::sqlserver_column_type_generator::SqlServerColumnTypeGenerator;
use schema_sql_generator::sqlserver::sqlserver_trigger_generator::SqlServerTriggerGenerator;

use crate::check_constraint;
use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

pub struct SqlServerMigrationGenerator;

impl MigrationGenerator for SqlServerMigrationGenerator {
    fn generate(
        &self,
        change_set: &ChangeSet,
        database_model: &DatabaseModel,
        writer: &mut dyn Write,
    ) -> Result<(), MigrationGeneratorError> {
        let context = GeneratorContext::for_model(Rc::new(database_model.clone()), DatabaseType::SqlServer);
        let type_generator = SqlServerColumnTypeGenerator::new(context.clone());
        let (trigger_context, trigger_buffer) =
            GeneratorContext::for_model_with_buffer(Rc::new(database_model.clone()), DatabaseType::SqlServer);
        let trigger_generator = SqlServerTriggerGenerator::new(trigger_context);
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();
        let mut triggers_regenerated: HashSet<String> = HashSet::new();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    write_guarded(
                        writer,
                        &format!("object_id('{}', 'U') is null", table_name),
                        &format!("create table {} ();", table_name),
                    )?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(
                        writer,
                        "if object_id('{}', 'U') is not null drop table {};",
                        table_name, table_name
                    )?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameTable { old_name, new_name } => {
                    write_guarded(
                        writer,
                        &format!("object_id('{}', 'U') is not null and object_id('{}', 'U') is null", old_name, new_name),
                        &format!("exec sp_rename '{}', '{}';", old_name, new_name),
                    )?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, column));
                    let not_null = if column.required() { " not null" } else { " null" };
                    let default = default_sql(database_model, column)
                        .map(|d| format!(" default {}", d))
                        .unwrap_or_default();
                    write_guarded(
                        writer,
                        &format!(
                            "not exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
                            table_name,
                            column.name()
                        ),
                        &format!(
                            "alter table {} add {}{}{}{};",
                            table_name,
                            column.name(),
                            type_sql,
                            not_null,
                            default
                        ),
                    )?;

                    if let Some(check_sql) = check_constraint::check_constraint_sql(&context, column) {
                        let name = check_constraint::constraint_name(table_name, column.name());
                        write_guarded(
                            writer,
                            &format!("not exists (select 1 from sys.check_constraints where name = '{}')", name),
                            &format!("alter table {} add constraint {} {};", table_name, name, check_sql),
                        )?;
                    }
                }
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   exec sp_rename '{}.{}', '{}', 'COLUMN';", table_name, column_name, candidate)?;
                        }
                    }
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
                            table_name, column_name
                        ),
                        &format!("alter table {} drop column {};", table_name, column_name),
                    )?;
                }
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}') and not exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
                            table_name, old_name, table_name, new_name
                        ),
                        &format!("exec sp_rename '{}.{}', '{}', 'COLUMN';", table_name, old_name, new_name),
                    )?;
                }
                SchemaChange::ModifyColumn { table_name, old_column: _, new_column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, new_column));
                    let null = if new_column.required() { " not null" } else { " null" };
                    writeln!(
                        writer,
                        "alter table {} alter column {}{}{};",
                        table_name,
                        new_column.name(),
                        type_sql,
                        null
                    )?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddKey { table_name, key, ordinal } => {
                    write_add_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::DropKey { table_name, key, ordinal } => {
                    write_drop_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::AddConstraint { table_name, constraint } => {
                    write_guarded(
                        writer,
                        &format!("not exists (select 1 from sys.check_constraints where name = '{}')", constraint.name()),
                        &format!(
                            "alter table {} add constraint {} check ({});",
                            table_name,
                            constraint.name(),
                            constraint.sql()
                        ),
                    )?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.objects where name = '{}' and parent_object_id = object_id('{}'))",
                            constraint_name, table_name
                        ),
                        &format!("alter table {} drop constraint {};", table_name, constraint_name),
                    )?;
                }
                SchemaChange::AddRelation { relation, ordinal } => {
                    write_add_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::DropRelation { relation, ordinal } => {
                    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), *ordinal);
                    write_guarded(
                        writer,
                        &format!("exists (select 1 from sys.foreign_keys where name = '{}')", fk_name),
                        &format!("alter table {} drop constraint {};", relation.from_table_name(), fk_name),
                    )?;
                }
                SchemaChange::AddView { view } => {
                    writeln!(writer, "create or alter view {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { view_name } => {
                    writeln!(
                        writer,
                        "if object_id('{}', 'V') is not null drop view {};",
                        view_name, view_name
                    )?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                // SQL Server has no native enum type (enums are emulated as a CHECK
                // constraint per column, same as SQLite) - nothing to do at the type level.
                SchemaChange::AddEnumType { .. } | SchemaChange::DropEnumType { .. } => {}
                SchemaChange::ModifyEnumType { old_enum_type, new_enum_type } => {
                    write_removed_enum_value_guards(writer, old_enum_type, new_enum_type)?;
                    for (table_name, column) in columns_using_enum(database_model, new_enum_type.name()) {
                        let name = check_constraint::constraint_name(&table_name, column.name());
                        write_guarded(
                            writer,
                            &format!("exists (select 1 from sys.check_constraints where name = '{}')", name),
                            &format!("alter table {} drop constraint {};", table_name, name),
                        )?;
                        if let Some(check_sql) = check_constraint::check_constraint_sql(&context, column) {
                            write_guarded(
                                writer,
                                &format!("not exists (select 1 from sys.check_constraints where name = '{}')", name),
                                &format!("alter table {} add constraint {} {};", table_name, name, check_sql),
                            )?;
                        }
                    }
                }
                SchemaChange::AddFunction { function } if function.database_type() == DatabaseType::SqlServer => {
                    writeln!(writer, "{}", function.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddFunction { .. } => {}
                SchemaChange::DropFunction { function_name, database_type } if *database_type == DatabaseType::SqlServer => {
                    writeln!(writer, "drop function if exists {};", function_name)?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropFunction { .. } => {}
                SchemaChange::AddProcedure { procedure } if procedure.database_type() == DatabaseType::SqlServer => {
                    writeln!(writer, "{}", procedure.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddProcedure { .. } => {}
                SchemaChange::DropProcedure { procedure_name, database_type } if *database_type == DatabaseType::SqlServer => {
                    writeln!(writer, "drop procedure if exists {};", procedure_name)?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropProcedure { .. } => {}
                SchemaChange::AddOtherSql { other_sql } if other_sql.database_type() == DatabaseType::SqlServer => {
                    writeln!(writer, "{}", other_sql.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddOtherSql { .. } => {}
                SchemaChange::DropOtherSql { other_sql } if other_sql.database_type() == DatabaseType::SqlServer => {
                    writeln!(writer, "-- TODO: other_sql entry removed, review manually: {}", other_sql.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropOtherSql { .. } => {}
                SchemaChange::AddTrigger { table_name, trigger } if trigger.database_type() == DatabaseType::SqlServer => {
                    regenerate_triggers(&trigger_generator, &trigger_buffer, database_model, table_name, &mut triggers_regenerated, &mut *writer)?;
                }
                SchemaChange::AddTrigger { .. } => {}
                SchemaChange::DropTrigger { table_name, trigger } if trigger.database_type() == DatabaseType::SqlServer => {
                    regenerate_triggers(&trigger_generator, &trigger_buffer, database_model, table_name, &mut triggers_regenerated, &mut *writer)?;
                }
                SchemaChange::DropTrigger { .. } => {}
                SchemaChange::AddInitialData { initial_data, .. }
                    if initial_data.database_type().is_none() || initial_data.database_type() == Some(DatabaseType::SqlServer) =>
                {
                    writeln!(writer, "{}", initial_data.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddInitialData { .. } => {}
                SchemaChange::DropInitialData { initial_data, .. }
                    if initial_data.database_type().is_none() || initial_data.database_type() == Some(DatabaseType::SqlServer) =>
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

/// Columns across every table in the model whose `enumType` matches `enum_name`
/// (case-insensitive) - used by `ModifyEnumType` to find every CHECK constraint that needs
/// regenerating for SQLite and SQL Server, which emulate enums as `varchar` + `CHECK (col IN
/// (...))` rather than a native type (see `DefaultColumnConstraintGenerator::enum_check_constraint_sql`).
fn columns_using_enum<'a>(database_model: &'a DatabaseModel, enum_name: &str) -> Vec<(String, &'a Column)> {
    database_model
        .all_tables()
        .into_iter()
        .flat_map(|table| {
            table
                .columns()
                .iter()
                .filter(|c| c.enum_type().is_some_and(|t| t.eq_ignore_ascii_case(enum_name)))
                .map(|c| (table.name().to_string(), c))
        })
        .collect()
}

/// Unlike Postgres, SQL Server's `ADD CONSTRAINT` already validates existing data by default
/// and would fail on its own if a row still held a removed value - but only when such a row
/// actually exists, and only with a generic constraint-violation error. A removed value is a
/// deliberate schema decision that deserves an explicit stop and explanation every time, not a
/// silent pass-through when no violating row happens to exist yet - so this emits an
/// unconditional `throw` before any of `ModifyEnumType`'s drop/add constraint statements run,
/// forcing the developer to either confirm the value is unused (delete the block) or add
/// `update` statements above it first.
fn write_removed_enum_value_guards(
    writer: &mut dyn Write,
    old_enum_type: &schema_model::model::enum_type::EnumType,
    new_enum_type: &schema_model::model::enum_type::EnumType,
) -> Result<(), MigrationGeneratorError> {
    let name = new_enum_type.name();

    for value in old_enum_type.values() {
        if !new_enum_type.values().iter().any(|v| v.name() == value.name()) {
            writeln!(writer, "-- WARNING: enum value '{}' was removed from '{}'.", value.name(), name)?;
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
            writeln!(
                writer,
                "throw 50000, 'Migration halted: enum value ''{}'' was removed from {} - see comment above.', 1;",
                value.name().replace('\'', "''"),
                name
            )?;
            writeln!(writer, "go")?;
            writeln!(writer)?;
        }
    }

    Ok(())
}

/// The `default` XML attribute is free-text SQL for every column type except `Boolean`,
/// which is authored as `true`/`false`/`yes`/`no`/etc and must be rendered as whatever
/// literal `boolean_mode` expects (`1`/`0` for `Native`'s `bit` type, `'Yes'`/`'No'` for
/// `YesNo`, etc) - see `DefaultColumnGenerator::convert_boolean_default_constraint` in
/// schema-sql-generator.
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
            BooleanMode::Native => if value { "1" } else { "0" },
            BooleanMode::YesNo => if value { "'Yes'" } else { "'No'" },
            BooleanMode::YN => if value { "'Y'" } else { "'N'" },
        }
        .to_string(),
    )
}

/// T-SQL has no `IF [NOT] EXISTS` clause on `ALTER TABLE`/`CREATE TABLE`/`sp_rename`, so
/// idempotency has to be expressed as an explicit `IF (NOT) EXISTS (...) BEGIN ... END`
/// guard around the statement, using the relevant `sys.*` catalog view.
fn write_guarded(writer: &mut dyn Write, condition_sql: &str, body_sql: &str) -> Result<(), MigrationGeneratorError> {
    writeln!(writer, "if {} begin", condition_sql)?;
    writeln!(writer, "  {}", body_sql)?;
    writeln!(writer, "end")?;
    writeln!(writer, "go")?;
    writeln!(writer)?;
    Ok(())
}

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let cols: String = key.columns().iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
    match key.key_type() {
        KeyType::Primary => {
            let constraint_name = primary_key_name(DatabaseType::SqlServer, table_name);
            write_guarded(
                writer,
                &format!(
                    "not exists (select 1 from sys.key_constraints where name = '{}' and parent_object_id = object_id('{}'))",
                    constraint_name, table_name
                ),
                &format!("alter table {} add constraint {} primary key ({});", table_name, constraint_name, cols),
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::SqlServer, table_name, ordinal);
            write_guarded(
                writer,
                &format!(
                    "not exists (select 1 from sys.key_constraints where name = '{}' and parent_object_id = object_id('{}'))",
                    constraint_name, table_name
                ),
                &format!("alter table {} add constraint {} unique ({});", table_name, constraint_name, cols),
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::SqlServer, table_name, ordinal);
            let unique = if key.is_unique() { "unique " } else { "" };
            let where_clause = key.filter().map(|f| format!(" where {}", f)).unwrap_or_default();
            write_guarded(
                writer,
                &format!("not exists (select 1 from sys.indexes where name = '{}')", idx_name),
                &format!("create {}index {} on {} ({}){};", unique, idx_name, table_name, cols, where_clause),
            )?;
        }
    }
    Ok(())
}

fn write_drop_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    match key.key_type() {
        KeyType::Primary => {
            let constraint_name = primary_key_name(DatabaseType::SqlServer, table_name);
            write_guarded(
                writer,
                &format!(
                    "exists (select 1 from sys.key_constraints where name = '{}' and parent_object_id = object_id('{}'))",
                    constraint_name, table_name
                ),
                &format!("alter table {} drop constraint {};", table_name, constraint_name),
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::SqlServer, table_name, ordinal);
            writeln!(
                writer,
                "if exists (select 1 from sys.key_constraints where name = '{}' and parent_object_id = object_id('{}')) alter table {} drop constraint {};",
                constraint_name, table_name, table_name, constraint_name
            )?;
            writeln!(writer, "go")?;
            writeln!(writer)?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::SqlServer, table_name, ordinal);
            writeln!(
                writer,
                "if exists (select 1 from sys.indexes where name = '{}') drop index {} on {};",
                idx_name, idx_name, table_name
            )?;
            writeln!(writer, "go")?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), ordinal);
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " on delete cascade",
        RelationType::SetNull => " on delete set null",
        RelationType::DoNothing => " on delete no action",
        RelationType::Enforce => "",
    };
    let from_columns = relation.column_pairs().iter().map(|(from, _)| from.as_str()).collect::<Vec<_>>().join(", ");
    let to_columns = relation.column_pairs().iter().map(|(_, to)| to.as_str()).collect::<Vec<_>>().join(", ");
    write_guarded(
        writer,
        &format!("not exists (select 1 from sys.foreign_keys where name = '{}')", fk_name),
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

/// See the identical helper in `postgresql/mod.rs` - same reasoning: the trigger generator
/// writes through a dedicated `BufferSink`, drained into `writer` right away so it lands in
/// the right place relative to the rest of the migration's output.
fn regenerate_triggers(
    trigger_generator: &SqlServerTriggerGenerator,
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
