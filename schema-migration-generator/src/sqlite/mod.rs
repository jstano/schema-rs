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
use schema_model::naming::{index_name, unique_key_name};
use schema_sql_generator::common::column_type_generator::ColumnTypeGenerator;
use schema_sql_generator::common::generator_context::{BufferSink, GeneratorContext};
use schema_sql_generator::common::table_generator::TableGenerator;
use schema_sql_generator::sqlite::sqlite_column_type_generator::SqliteColumnTypeGenerator;
use schema_sql_generator::sqlite::sqlite_table_generator::SqliteTableGenerator;

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
        let (table_context, table_buffer) = GeneratorContext::for_model_with_buffer(Rc::new(database_model.clone()), DatabaseType::Sqlite);
        let table_generator = SqliteTableGenerator::new(table_context);
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();

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
                    // SQLite's `ADD COLUMN` rejects `NOT NULL` outright unless a non-null
                    // `DEFAULT` is also given - a hard syntax-level restriction, not a runtime
                    // failure on non-empty tables like Postgres/SQL Server, so it fails even
                    // against an empty table. With no default to fall back on, and no `ALTER
                    // COLUMN` to tighten it afterwards (SQLite needs a full table rebuild for
                    // that - see `ModifyColumn` below), the column has to go in nullable, with
                    // enforcing `NOT NULL` left to a manual rebuild.
                    let needs_manual_not_null = column.required() && default_value.is_none();
                    let not_null = if column.required() && !needs_manual_not_null { " not null" } else { "" };
                    let default = default_value.map(|d| format!(" default {}", d)).unwrap_or_default();
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
                    if needs_manual_not_null {
                        writeln!(
                            writer,
                            "-- WARNING: '{}' should be NOT NULL, but SQLite's ADD COLUMN requires a non-null DEFAULT to add a NOT NULL column, and none is set here.",
                            column.name()
                        )?;
                        writeln!(
                            writer,
                            "-- Backfill '{}' and manually recreate the table to enforce NOT NULL - SQLite has no ALTER COLUMN.",
                            column.name()
                        )?;
                        writeln!(writer)?;
                    }
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
                SchemaChange::AddConstraint { table_name, constraint } if constraint.database_type() == DatabaseType::Sqlite => {
                    writeln!(
                        writer,
                        "-- SQLite does not support adding constraint '{}' to table '{}' in-place.",
                        constraint.name(),
                        table_name
                    )?;
                    writeln!(writer, "-- Manually recreate the table with the constraint.")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddConstraint { .. } => {}
                SchemaChange::DropConstraint { table_name, constraint_name, database_type } if *database_type == DatabaseType::Sqlite => {
                    writeln!(
                        writer,
                        "-- SQLite does not support dropping constraint '{}' from table '{}' in-place.",
                        constraint_name,
                        table_name
                    )?;
                    writeln!(writer, "-- Manually recreate the table without the constraint.")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropConstraint { .. } => {}
                SchemaChange::AddRelation { relation, .. } => {
                    write_add_relation(writer, relation)?;
                }
                SchemaChange::DropRelation { relation, .. } => {
                    let from_columns = relation.column_pairs().iter().map(|(from, _)| from.as_str()).collect::<Vec<_>>().join(", ");
                    writeln!(
                        writer,
                        "-- SQLite does not support dropping foreign key on '{}.({})'.",
                        relation.from_table_name(),
                        from_columns
                    )?;
                    writeln!(writer, "-- Manually recreate the table without this foreign key.")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { view }
                    if view.database_type().is_none() || view.database_type() == Some(DatabaseType::Sqlite) =>
                {
                    writeln!(writer, "create view if not exists {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { .. } => {}
                SchemaChange::DropView { view_name, database_type }
                    if database_type.is_none() || *database_type == Some(DatabaseType::Sqlite) =>
                {
                    writeln!(writer, "drop view if exists {};", view_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { .. } => {}
                // SQLite has no native enum type (enums are emulated as a CHECK constraint
                // per column) - nothing to do at the type level.
                SchemaChange::AddEnumType { .. } | SchemaChange::DropEnumType { .. } => {}
                SchemaChange::ModifyEnumType { new_enum_type, .. } => {
                    let affected = columns_using_enum(database_model, new_enum_type.name());
                    if !affected.is_empty() {
                        writeln!(
                            writer,
                            "-- SQLite does not support altering a CHECK constraint in-place; enum '{}' changed.",
                            new_enum_type.name()
                        )?;
                        writeln!(writer, "-- Manually recreate the following tables with the updated value list:")?;
                        for (table_name, column) in affected {
                            writeln!(writer, "--   {}.{}", table_name, column.name())?;
                        }
                        writeln!(writer)?;
                    }
                }
                SchemaChange::AddFunction { function } if function.database_type() == DatabaseType::Sqlite => {
                    writeln!(writer, "{};", function.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddFunction { .. } => {}
                SchemaChange::DropFunction { function_name, database_type } if *database_type == DatabaseType::Sqlite => {
                    writeln!(writer, "-- SQLite has no generic DROP FUNCTION; function '{}' removed - review manually.", function_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropFunction { .. } => {}
                SchemaChange::AddProcedure { procedure } if procedure.database_type() == DatabaseType::Sqlite => {
                    writeln!(writer, "{};", procedure.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddProcedure { .. } => {}
                SchemaChange::DropProcedure { procedure_name, database_type } if *database_type == DatabaseType::Sqlite => {
                    writeln!(writer, "-- SQLite has no generic DROP PROCEDURE; procedure '{}' removed - review manually.", procedure_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropProcedure { .. } => {}
                SchemaChange::AddOtherSql { other_sql } if other_sql.database_type() == DatabaseType::Sqlite => {
                    writeln!(writer, "{};", other_sql.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddOtherSql { .. } => {}
                SchemaChange::DropOtherSql { other_sql } if other_sql.database_type() == DatabaseType::Sqlite => {
                    writeln!(writer, "-- TODO: other_sql entry removed, review manually: {}", other_sql.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropOtherSql { .. } => {}
                // SQLite trigger generation is a no-op in schema-sql-generator (see
                // `SqliteTriggerGenerator`); nothing to regenerate here either.
                SchemaChange::AddTrigger { .. } | SchemaChange::DropTrigger { .. } => {}
                SchemaChange::AddInitialData { initial_data, .. }
                    if initial_data.database_type().is_none() || initial_data.database_type() == Some(DatabaseType::Sqlite) =>
                {
                    writeln!(writer, "{};", initial_data.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::AddInitialData { .. } => {}
                SchemaChange::DropInitialData { initial_data, .. }
                    if initial_data.database_type().is_none() || initial_data.database_type() == Some(DatabaseType::Sqlite) =>
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
/// `schema-sql-generator`'s own `SqliteTableGenerator` (C7) - the same column/key/constraint
/// type mapping the full-schema generator uses, including inline foreign keys (SQLite has no
/// `ALTER TABLE ... ADD CONSTRAINT`, so `SqliteTableGenerator::output_table_definition` already
/// embeds them) - rather than the previous hand-rolled, always-empty `create table (id integer
/// primary key autoincrement)`. Only `output_table_definition`/`output_indexes`/
/// `output_initial_data` are called, not `output_table` itself - see the matching Postgres
/// helper's doc comment for why. Relations are still separately diffed as `AddRelation` (see
/// `diff_add_relations` in schema-diff) purely so `write_add_relation`'s informational comment
/// below fires and points at this table's create statement; it does not emit any DDL here.
fn write_add_table(
    table_generator: &SqliteTableGenerator,
    table_buffer: &BufferSink,
    table: &Table,
    writer: &mut dyn Write,
) -> Result<(), MigrationGeneratorError> {
    let qualified_name = table.fully_qualified_table_name(DatabaseType::Sqlite);

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

/// Columns across every table in the model whose `enumType` matches `enum_name`
/// (case-insensitive) - used by `ModifyEnumType` to list which CHECK constraints need a
/// manual rebuild (SQLite emulates enums as `varchar` + `CHECK (col IN (...))`, same as SQL
/// Server, but can't alter a CHECK constraint in place - see the `AddConstraint`/
/// `DropConstraint` arms above).
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
            let unique = if key.is_unique() { "unique " } else { "" };
            let where_clause = key.filter().map(|f| format!(" where {}", f)).unwrap_or_default();
            writeln!(
                writer,
                "create {}index if not exists {} on {} ({}){};",
                unique, idx_name, table_name, cols, where_clause
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
    let from_columns = relation.column_pairs().iter().map(|(from, _)| from.as_str()).collect::<Vec<_>>().join(", ");
    let to_columns = relation.column_pairs().iter().map(|(_, to)| to.as_str()).collect::<Vec<_>>().join(", ");
    writeln!(
        writer,
        "-- SQLite foreign keys must be declared at table creation time."
    )?;
    writeln!(
        writer,
        "-- Ensure foreign key ({}) references {}({}){} is in the create table statement for '{}'.",
        from_columns,
        relation.to_table_name(),
        to_columns,
        on_delete,
        relation.from_table_name()
    )?;
    writeln!(writer)?;
    Ok(())
}
