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
use schema_model::naming::{index_name, unique_key_name};
use schema_sql_generator::common::column_type_generator::ColumnTypeGenerator;
use schema_sql_generator::common::generator_context::GeneratorContext;
use schema_sql_generator::sqlite::sqlite_column_type_generator::SqliteColumnTypeGenerator;

use crate::check_constraint;
use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

pub struct SqliteMigrationGenerator;

impl MigrationGenerator for SqliteMigrationGenerator {
    fn generate(
        &self,
        change_set: &ChangeSet,
        database_model: &DatabaseModel,
        writer: &mut dyn Write,
    ) -> Result<(), MigrationGeneratorError> {
        let context = GeneratorContext::for_model(Rc::new(database_model.clone()), DatabaseType::Sqlite);
        let type_generator = SqliteColumnTypeGenerator::new(context.clone());
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    writeln!(writer, "create table if not exists {} (id integer primary key autoincrement);", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(writer, "drop table if exists {};", table_name)?;
                    writeln!(writer)?;
                }
                // SQLite has no `IF EXISTS`/conditional-DDL form for `RENAME TO` and no
                // procedural `IF`/`DO` block at the plain-SQL level, so this cannot be made
                // idempotent: re-running it after `old_name` has already been renamed away
                // will error.
                SchemaChange::RenameTable { old_name, new_name } => {
                    writeln!(writer, "alter table {} rename to {};", old_name, new_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, column));
                    let not_null = if column.required() { " not null" } else { "" };
                    let default = default_sql(database_model, column)
                        .map(|d| format!(" default {}", d))
                        .unwrap_or_default();
                    // SQLite has no `alter table ... add constraint` (see the `AddConstraint`
                    // arm below), so a new column's CHECK constraint must be declared inline
                    // in the column definition instead of as a separate statement.
                    let check = check_constraint::check_constraint_sql(&context, column)
                        .map(|check_sql| format!(" {}", check_sql))
                        .unwrap_or_default();
                    // SQLite's `ALTER TABLE ... ADD COLUMN` has no `IF NOT EXISTS` form and
                    // there is no procedural guard available in plain SQL, so this cannot be
                    // made idempotent: re-running it after the column already exists will error.
                    writeln!(
                        writer,
                        "alter table {} add column {}{}{}{}{};",
                        table_name,
                        column.name(),
                        type_sql,
                        not_null,
                        check,
                        default
                    )?;
                    writeln!(writer)?;
                }
                // SQLite does not support DROP COLUMN before version 3.35.0.
                // Generate a comment noting a manual table-rebuild may be needed.
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   alter table {} rename column {} to {};", table_name, column_name, candidate)?;
                        }
                    }
                    writeln!(
                        writer,
                        "-- SQLite 3.35+: alter table {} drop column {};",
                        table_name, column_name
                    )?;
                    writeln!(writer, "-- For older SQLite: manually recreate the table without this column.")?;
                    writeln!(writer)?;
                }
                // SQLite has no conditional-DDL syntax to guard this at the plain-SQL level,
                // so re-running it after the column has already been renamed will error.
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    writeln!(
                        writer,
                        "alter table {} rename column {} to {};",
                        table_name, old_name, new_name
                    )?;
                    writeln!(writer)?;
                }
                // SQLite does not support ALTER COLUMN — requires table rebuild.
                SchemaChange::ModifyColumn { table_name, old_column: _, new_column } => {
                    writeln!(
                        writer,
                        "-- SQLite does not support modifying column '{}' on table '{}' in-place.",
                        new_column.name(),
                        table_name
                    )?;
                    writeln!(writer, "-- Manually recreate the table with the updated column definition.")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddKey { table_name, key, ordinal } => {
                    write_add_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::DropKey { table_name, key, ordinal } => {
                    write_drop_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::AddConstraint { table_name, constraint } => {
                    writeln!(
                        writer,
                        "-- SQLite does not support adding constraint '{}' to table '{}' in-place.",
                        constraint.name(),
                        table_name
                    )?;
                    writeln!(writer, "-- Manually recreate the table with the constraint.")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    writeln!(
                        writer,
                        "-- SQLite does not support dropping constraint '{}' from table '{}' in-place.",
                        constraint_name,
                        table_name
                    )?;
                    writeln!(writer, "-- Manually recreate the table without the constraint.")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddRelation { relation, .. } => {
                    write_add_relation(writer, relation)?;
                }
                SchemaChange::DropRelation { relation, .. } => {
                    writeln!(
                        writer,
                        "-- SQLite does not support dropping foreign key on '{}.{}'.",
                        relation.from_table_name(),
                        relation.from_column_name()
                    )?;
                    writeln!(writer, "-- Manually recreate the table without this foreign key.")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { view } => {
                    writeln!(writer, "create view if not exists {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { view_name } => {
                    writeln!(writer, "drop view if exists {};", view_name)?;
                    writeln!(writer)?;
                }
            }
        }
        Ok(())
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

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let cols: String = key.columns().iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
    match key.key_type() {
        KeyType::Primary => {
            writeln!(
                writer,
                "-- SQLite does not support adding a PRIMARY KEY constraint in-place on table '{}'.",
                table_name
            )?;
            writeln!(writer, "-- Manually recreate the table with the primary key.")?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Sqlite, table_name, ordinal);
            writeln!(
                writer,
                "create unique index if not exists {} on {} ({});",
                constraint_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Sqlite, table_name, ordinal);
            writeln!(
                writer,
                "create index if not exists {} on {} ({});",
                idx_name, table_name, cols
            )?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_drop_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    match key.key_type() {
        KeyType::Primary => {
            writeln!(
                writer,
                "-- SQLite does not support dropping a PRIMARY KEY constraint in-place on table '{}'.",
                table_name
            )?;
            writeln!(writer, "-- Manually recreate the table without the primary key.")?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Sqlite, table_name, ordinal);
            writeln!(writer, "drop index if exists {};", constraint_name)?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Sqlite, table_name, ordinal);
            writeln!(writer, "drop index if exists {};", idx_name)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation) -> Result<(), MigrationGeneratorError> {
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " on delete cascade",
        RelationType::SetNull => " on delete set null",
        RelationType::DoNothing => " on delete restrict",
        RelationType::Enforce => "",
    };
    writeln!(
        writer,
        "-- SQLite foreign keys must be declared at table creation time."
    )?;
    writeln!(
        writer,
        "-- Ensure foreign key ({}) references {}({}){} is in the create table statement for '{}'.",
        relation.from_column_name(),
        relation.to_table_name(),
        relation.to_column_name(),
        on_delete,
        relation.from_table_name()
    )?;
    writeln!(writer)?;
    Ok(())
}
