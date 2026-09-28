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
use schema_sql_generator::common::table_generator::TableGenerator;
use schema_sql_generator::common::trigger_generator::TriggerGenerator;
use schema_sql_generator::sqlserver::sqlserver_column_type_generator::SqlServerColumnTypeGenerator;
use schema_sql_generator::sqlserver::sqlserver_table_generator::SqlServerTableGenerator;
use schema_sql_generator::sqlserver::sqlserver_trigger_generator::SqlServerTriggerGenerator;

use crate::check_constraint;
use crate::default_constraint;
use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

#[derive(Default)]
pub struct SqlServerMigrationGenerator {
    /// Target SQL Server product year (e.g. 2022, 2025) - threaded into every
    /// `GeneratorContext` this generator builds so version-gated column type SQL (e.g. the
    /// native `json` type, M6) matches what `--sqlserver-version` would produce on the create
    /// path. `0` means "unset".
    pub target_sqlserver_version: u32,
}

impl SqlServerMigrationGenerator {
    pub fn new(target_sqlserver_version: u32) -> Self {
        Self { target_sqlserver_version }
    }
}

impl MigrationGenerator for SqlServerMigrationGenerator {
    fn generate(
        &self,
        change_set: &ChangeSet,
        database_model: &DatabaseModel,
        writer: &mut dyn Write,
    ) -> Result<(), MigrationGeneratorError> {
        let context = GeneratorContext::for_model_with_target_version(
            Rc::new(database_model.clone()),
            DatabaseType::SqlServer,
            self.target_sqlserver_version,
        );
        let type_generator = SqlServerColumnTypeGenerator::new(context.clone());
        let (trigger_context, trigger_buffer) = GeneratorContext::for_model_with_buffer_and_target_version(
            Rc::new(database_model.clone()),
            DatabaseType::SqlServer,
            self.target_sqlserver_version,
        );
        let trigger_generator = SqlServerTriggerGenerator::new(trigger_context);
        let (table_context, table_buffer) = GeneratorContext::for_model_with_buffer_and_target_version(
            Rc::new(database_model.clone()),
            DatabaseType::SqlServer,
            self.target_sqlserver_version,
        );
        let table_generator = SqlServerTableGenerator::new(table_context);
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();
        let mut triggers_regenerated: HashSet<String> = HashSet::new();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table } => {
                    write_add_table(&table_generator, &table_buffer, table, writer)?;
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
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, column));
                    let default_value = default_sql(database_model, column);

                    if column.required() && default_value.is_none() {
                        write_not_null_column_with_backfill_guard(writer, table_name, column.name(), &type_sql)?;
                    } else {
                        let not_null = if column.required() { " not null" } else { " null" };
                        let default = default_value
                            .map(|d| {
                                let name = default_constraint::constraint_name(table_name, column.name());
                                format!(" constraint {} default {}", name, d)
                            })
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
                    }

                    write_add_check_constraint(writer, &context, table_name, column)?;
                }
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   exec sp_rename '{}.{}', '{}', 'COLUMN';", table_name, column_name, candidate)?;
                        }
                    }
                    write_drop_default_constraint(writer, table_name, column_name)?;
                    write_drop_check_constraint(writer, table_name, column_name)?;
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
                            table_name, column_name
                        ),
                        &format!("alter table {} drop column {};", table_name, column_name),
                    )?;
                }
                SchemaChange::ModifyColumn { table_name, old_column, new_column } => {
                    // SQL Server also refuses ALTER COLUMN outright while the column
                    // participates in a PRIMARY KEY/UNIQUE constraint, a plain index, or a
                    // foreign key (either as the FK's own column, or as the column another
                    // table's FK references) - unlike DEFAULT/CHECK, there's no widening
                    // exception for any of these. Find and drop them first, guarded, in
                    // dependency order (a relation can depend on a unique/PK index, so it has
                    // to come off first and go back on last).
                    let table = database_model.all_tables().into_iter().find(|t| t.name().eq_ignore_ascii_case(table_name));
                    let blocking_keys = table.map(|t| dependent_keys(t, old_column.name())).unwrap_or_default();
                    let blocking_relations = dependent_relations(database_model, table_name, old_column.name());

                    for (ordinal, relation) in &blocking_relations {
                        write_drop_relation(writer, relation, *ordinal)?;
                    }
                    for (key, ordinal) in &blocking_keys {
                        write_drop_key(writer, table_name, key, *ordinal)?;
                    }

                    // SQL Server refuses ALTER COLUMN while a DEFAULT or CHECK constraint from
                    // the old definition still references the column (the same restriction
                    // DROP COLUMN has) - drop both first, guarded, keyed off `old_column`'s
                    // name (same as `new_column`'s, since this isn't a rename).
                    write_drop_default_constraint(writer, table_name, old_column.name())?;
                    write_drop_check_constraint(writer, table_name, old_column.name())?;

                    // IDENTITY can only be set when a column is created - ALTER COLUMN rejects
                    // an `identity(...)` clause outright, so `type_generator`'s normal
                    // `integer identity(1,1)`/`bigint identity(1,1)` output (see
                    // `sequence_sql`/`long_sequence_sql`) is a syntax error here. Strip it down
                    // to the bare integer type; this can still change nullability, but not
                    // retarget an identity column's underlying type - that needs a manual
                    // column rebuild, so flag it instead of emitting DDL that would either fail
                    // or silently do the wrong thing.
                    let is_identity = matches!(new_column.column_type(), ColumnType::Sequence | ColumnType::LongSequence);
                    if is_identity {
                        writeln!(
                            writer,
                            "-- NOTE: '{}' is an identity column - IDENTITY cannot be changed via ALTER COLUMN. If its underlying type needs to change too, this requires a manual column rebuild.",
                            new_column.name()
                        )?;
                    }
                    let type_sql = if is_identity {
                        match new_column.column_type() {
                            ColumnType::Sequence => " integer".to_string(),
                            ColumnType::LongSequence => " bigint".to_string(),
                            _ => unreachable!(),
                        }
                    } else {
                        format!(" {}", type_generator.column_type_sql(&dummy_table, new_column))
                    };
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

                    write_add_default_constraint(writer, database_model, table_name, new_column)?;
                    write_add_check_constraint(writer, &context, table_name, new_column)?;

                    for (key, ordinal) in &blocking_keys {
                        write_add_key(writer, table_name, key, *ordinal)?;
                    }
                    for (ordinal, relation) in &blocking_relations {
                        write_add_relation(writer, relation, *ordinal)?;
                    }
                }
                SchemaChange::AddKey { table_name, key, ordinal } => {
                    write_add_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::DropKey { table_name, key, ordinal } => {
                    write_drop_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::AddConstraint { table_name, constraint } if constraint.database_type() == DatabaseType::SqlServer => {
                    // `constraint.sql()` is already the full `check (...)` clause - same
                    // convention as `DefaultTableConstraintGenerator` on the create path
                    // (`constraint {name} {sql}`, no wrapper added) - so this must not
                    // re-wrap it, or it comes out as `check (check (...))`.
                    write_guarded(
                        writer,
                        &format!("not exists (select 1 from sys.check_constraints where name = '{}')", constraint.name()),
                        &format!(
                            "alter table {} add constraint {} {};",
                            table_name,
                            constraint.name(),
                            constraint.sql()
                        ),
                    )?;
                }
                SchemaChange::AddConstraint { .. } => {}
                SchemaChange::DropConstraint { table_name, constraint_name, database_type } if *database_type == DatabaseType::SqlServer => {
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.objects where name = '{}' and parent_object_id = object_id('{}'))",
                            constraint_name, table_name
                        ),
                        &format!("alter table {} drop constraint {};", table_name, constraint_name),
                    )?;
                }
                SchemaChange::DropConstraint { .. } => {}
                SchemaChange::AddRelation { relation, ordinal } => {
                    write_add_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::DropRelation { relation, ordinal } => {
                    write_drop_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::AddView { view }
                    if view.database_type().is_none() || view.database_type() == Some(DatabaseType::SqlServer) =>
                {
                    writeln!(writer, "create or alter view {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { .. } => {}
                SchemaChange::DropView { view_name, database_type }
                    if database_type.is_none() || *database_type == Some(DatabaseType::SqlServer) =>
                {
                    writeln!(
                        writer,
                        "if object_id('{}', 'V') is not null drop view {};",
                        view_name, view_name
                    )?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { .. } => {}
                // SQL Server has no native enum type (enums are emulated as a CHECK
                // constraint per column, same as SQLite) - nothing to do at the type level.
                SchemaChange::AddEnumType { .. } | SchemaChange::DropEnumType { .. } => {}
                SchemaChange::ModifyEnumType { old_enum_type, new_enum_type } => {
                    write_removed_enum_value_guards(writer, old_enum_type, new_enum_type)?;
                    for (table_name, column) in columns_using_enum(database_model, new_enum_type.name()) {
                        write_drop_check_constraint(writer, &table_name, column.name())?;
                        write_add_check_constraint(writer, &context, &table_name, column)?;
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

/// `column_name`'s own PRIMARY KEY/UNIQUE keys and indexes on `table`, paired with the
/// 1-based per-category ordinal the create path would number them with (see
/// `unique_key_name`/`index_name`) - mirrors `schema_diff::diff_engine`'s own per-category
/// counting so a key dropped and re-added here gets the exact name a fresh install would give
/// it. A `KeyType::Primary` entry's ordinal is unused (`primary_key_name` ignores it) and left
/// at whatever the unique-key counter happens to be, same as the diff engine.
fn dependent_keys<'a>(table: &'a Table, column_name: &str) -> Vec<(&'a Key, usize)> {
    let mut found = Vec::new();

    let mut unique_ordinal = 0;
    for key in table.keys() {
        if key.key_type() == KeyType::Unique {
            unique_ordinal += 1;
        }
        if key.contains_column(column_name) {
            found.push((key, unique_ordinal));
        }
    }

    let mut index_ordinal = 0;
    for key in table.indexes() {
        if key.is_index() {
            index_ordinal += 1;
        }
        if key.contains_column(column_name) {
            found.push((key, index_ordinal));
        }
    }

    found
}

/// Every foreign key anywhere in the model that touches `table_name`.`column_name` - either as
/// the FK's own (owning-table) column, or as the column another table's FK references - paired
/// with the relation's 1-based ordinal among its owning table's own `relations()` (see
/// `DefaultRelationGenerator::relation_constraint_name`), so `foreign_key_name` reproduces the
/// exact name the create path (or `AddRelation`/`DropRelation`) would give it. SQL Server blocks
/// `ALTER COLUMN` for both roles - the referencing column and the column being referenced - with
/// no exception, so both need to be found and cycled around a `ModifyColumn`.
fn dependent_relations<'a>(database_model: &'a DatabaseModel, table_name: &str, column_name: &str) -> Vec<(usize, &'a Relation)> {
    let mut found = Vec::new();

    for owning_table in database_model.all_tables() {
        for (index, relation) in owning_table.relations().iter().enumerate() {
            let ordinal = index + 1;
            let from_matches = owning_table.name().eq_ignore_ascii_case(table_name)
                && relation.column_pairs().iter().any(|(from, _)| from.eq_ignore_ascii_case(column_name));
            let to_matches = database_model
                .find_table_by_qualified_name_checked(relation.to_table_name())
                .is_some_and(|t| t.name().eq_ignore_ascii_case(table_name))
                && relation.column_pairs().iter().any(|(_, to)| to.eq_ignore_ascii_case(column_name));
            if from_matches || to_matches {
                found.push((ordinal, relation));
            }
        }
    }

    found
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

/// A required column with no `default` has no value to backfill existing rows with -
/// `alter table ... add col type not null` would just fail with a generic constraint-violation
/// error the moment the table has any rows (same reasoning as
/// `write_removed_enum_value_guards`: an unconditional halt every time, not a silent
/// pass-through when the table happens to be empty). Adds the column nullable, forces an
/// explicit developer decision (backfill it or confirm the table is empty), then tightens it
/// to `NOT NULL` - reachable only once the guard above it has been deleted.
fn write_not_null_column_with_backfill_guard(
    writer: &mut dyn Write,
    table_name: &str,
    column_name: &str,
    type_sql: &str,
) -> Result<(), MigrationGeneratorError> {
    write_guarded(
        writer,
        &format!(
            "not exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
            table_name, column_name
        ),
        &format!("alter table {} add {}{} null;", table_name, column_name, type_sql),
    )?;
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
    writeln!(
        writer,
        "throw 50000, 'Migration halted: {} has no value for {} - see comment above.', 1;",
        table_name, column_name
    )?;
    writeln!(writer, "go")?;
    writeln!(writer)?;
    writeln!(writer, "alter table {} alter column {}{} not null;", table_name, column_name, type_sql)?;
    writeln!(writer, "go")?;
    writeln!(writer)?;
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

/// SQL Server refuses `DROP COLUMN` and `ALTER COLUMN` while a `DEFAULT`/`CHECK` constraint
/// still references the column - drop each by its computed name first, guarded so this is a
/// safe no-op when it doesn't exist (the caller may not know either way, e.g. `DropColumn`
/// carries no `Column`/type info).
fn write_drop_default_constraint(writer: &mut dyn Write, table_name: &str, column_name: &str) -> Result<(), MigrationGeneratorError> {
    let name = default_constraint::constraint_name(table_name, column_name);
    write_guarded(
        writer,
        &format!("exists (select 1 from sys.default_constraints where name = '{}')", name),
        &format!("alter table {} drop constraint {};", table_name, name),
    )
}

fn write_drop_check_constraint(writer: &mut dyn Write, table_name: &str, column_name: &str) -> Result<(), MigrationGeneratorError> {
    let name = check_constraint::constraint_name(table_name, column_name);
    write_guarded(
        writer,
        &format!("exists (select 1 from sys.check_constraints where name = '{}')", name),
        &format!("alter table {} drop constraint {};", table_name, name),
    )
}

/// Re-creates `column`'s `DEFAULT` constraint (a no-op if it has none) as a separate `ADD
/// CONSTRAINT ... FOR <column>`, since (unlike `AddColumn`, which can inline the default into
/// the same statement that creates the column) `ALTER COLUMN` has no clause for it.
fn write_add_default_constraint(
    writer: &mut dyn Write,
    database_model: &DatabaseModel,
    table_name: &str,
    column: &Column,
) -> Result<(), MigrationGeneratorError> {
    if let Some(default) = default_sql(database_model, column) {
        let name = default_constraint::constraint_name(table_name, column.name());
        write_guarded(
            writer,
            &format!("not exists (select 1 from sys.default_constraints where name = '{}')", name),
            &format!("alter table {} add constraint {} default {} for {};", table_name, name, default, column.name()),
        )?;
    }
    Ok(())
}

fn write_add_check_constraint(
    writer: &mut dyn Write,
    context: &GeneratorContext,
    table_name: &str,
    column: &Column,
) -> Result<(), MigrationGeneratorError> {
    if let Some(check_sql) = check_constraint::check_constraint_sql(context, column) {
        let name = check_constraint::constraint_name(table_name, column.name());
        write_guarded(
            writer,
            &format!("not exists (select 1 from sys.check_constraints where name = '{}')", name),
            &format!("alter table {} add constraint {} {};", table_name, name, check_sql),
        )?;
    }
    Ok(())
}

/// T-SQL has no `IF [NOT] EXISTS` clause on `ALTER TABLE`/`CREATE TABLE`/`sp_rename`, so
/// idempotency has to be expressed as an explicit `IF (NOT) EXISTS (...) BEGIN ... END`
/// guard around the statement, using the relevant `sys.*` catalog view.
/// Renders a brand-new table's `CREATE TABLE` by routing it through
/// `schema-sql-generator`'s own `SqlServerTableGenerator` (C7) - the same column/key/constraint
/// type mapping the full-schema generator uses - rather than the previous hand-rolled, always-
/// empty `create table ()` (a hard T-SQL syntax error). Only `output_table_definition` is
/// called, not `output_table` itself - see the matching Postgres helper's doc comment for why
/// `output_table_drop`/`output_table_header` are skipped. Indexes are deliberately *not* taken
/// from `output_indexes`: that renders each `create index` already terminated with its own
/// `go` (correct for the full-schema generator, where every statement is its own top-level
/// batch), but `go` isn't valid *inside* the `begin ... end` this function's `write_guarded`
/// wraps the `CREATE TABLE` in - it would end the batch early and leave a dangling `end` with
/// no matching `begin`. So each index is instead rendered through the already-existing
/// `write_add_key` (used for `AddKey`), which produces its own separate, correctly-guarded
/// top-level statement - harmless redundancy for a table that doesn't exist yet, but avoids
/// the nesting bug. Initial data has no such problem (each row is already a standalone,
/// self-terminated statement - see `output_initial_data`), so it's rendered directly. Relations
/// and triggers are deliberately not included here at all - they're always a separate
/// generation pass, even for an existing table - see `diff_add_relations`/`diff_add_triggers`
/// in `schema-diff`, which still emit `AddRelation`/`AddTrigger` for a brand-new table, handled
/// by the existing arms below.
fn write_add_table(
    table_generator: &SqlServerTableGenerator,
    table_buffer: &BufferSink,
    table: &Table,
    writer: &mut dyn Write,
) -> Result<(), MigrationGeneratorError> {
    let qualified_name = table.fully_qualified_table_name(DatabaseType::SqlServer);

    table_generator.output_table_definition(table);
    let body = table_buffer.take();
    let create_sql = format!("create table {} (\n{}\n);", qualified_name, body.trim_end_matches('\n'));

    // Schema-qualified, matching `qualified_name` above - a bare `table.name()` would be
    // schema-blind (the same bug H15 fixed for SQL Server's drop guards: a same-named table
    // in a different schema, e.g. `dbo.orders`, would falsely satisfy the guard and skip
    // creating `s.orders` entirely).
    write_guarded(writer, &format!("object_id('{}', 'U') is null", qualified_name), &create_sql)?;

    // See diff_add_keys - a brand-new table's keys/indexes are skipped there (they're already
    // covered by `output_table_definition` above for primary/unique) except indexes, which are
    // never embedded inline and must still be created here, one at a time. Not routed through
    // `write_add_key`: its naming must stay keyed off the bare `table.name()` (matching what
    // the create path itself names the index - `index_name` bakes the table name straight into
    // the identifier, so a schema-qualified name would produce an invalid `ix_s.orders1`), but
    // the `on`/guard clauses must use `qualified_name` (an unqualified `on orders` would target
    // the wrong table if "orders" isn't in the connection's default schema) - two different
    // names `write_add_key` doesn't distinguish between.
    let mut index_ordinal = 0;
    for index in table.indexes() {
        if index.is_index() {
            index_ordinal += 1;
        }
        let idx_name = index_name(DatabaseType::SqlServer, table.name(), index_ordinal);
        let unique = if index.is_unique() { "unique " } else { "" };
        let cols: String = index.columns().iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
        let where_clause = index.filter().map(|f| format!(" where {}", f)).unwrap_or_default();
        write_guarded(
            writer,
            &format!("not exists (select 1 from sys.indexes where name = '{}')", idx_name),
            &format!("create {}index {} on {} ({}){};", unique, idx_name, qualified_name, cols, where_clause),
        )?;
    }

    table_generator.output_initial_data(table);
    write!(writer, "{}", table_buffer.take())?;
    Ok(())
}

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

fn write_drop_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), ordinal);
    write_guarded(
        writer,
        &format!("exists (select 1 from sys.foreign_keys where name = '{}')", fk_name),
        &format!("alter table {} drop constraint {};", relation.from_table_name(), fk_name),
    )
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
