use std::io::Write;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::types::{BooleanMode, DatabaseType, KeyType, RelationType};
use schema_model::naming::{index_name, unique_key_name};

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
        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    writeln!(writer, "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT);", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(writer, "DROP TABLE IF EXISTS {};", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameTable { old_name, new_name } => {
                    writeln!(writer, "ALTER TABLE {} RENAME TO {};", old_name, new_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = column_type_sql(database_model, column);
                    let not_null = if column.required() { " NOT NULL" } else { "" };
                    let default = default_sql(database_model, column)
                        .map(|d| format!(" DEFAULT {}", d))
                        .unwrap_or_default();
                    writeln!(
                        writer,
                        "ALTER TABLE {} ADD COLUMN {}{}{}{};",
                        table_name,
                        column.name(),
                        type_sql,
                        not_null,
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
                            writeln!(writer, "--   ALTER TABLE {} RENAME COLUMN {} TO {};", table_name, column_name, candidate)?;
                        }
                    }
                    writeln!(
                        writer,
                        "-- SQLite 3.35+: ALTER TABLE {} DROP COLUMN {};",
                        table_name, column_name
                    )?;
                    writeln!(writer, "-- For older SQLite: manually recreate the table without this column.")?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    writeln!(
                        writer,
                        "ALTER TABLE {} RENAME COLUMN {} TO {};",
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
                    writeln!(writer, "CREATE VIEW IF NOT EXISTS {} AS", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { view_name } => {
                    writeln!(writer, "DROP VIEW IF EXISTS {};", view_name)?;
                    writeln!(writer)?;
                }
            }
        }
        Ok(())
    }
}

fn column_type_sql(database_model: &DatabaseModel, column: &Column) -> String {
    match column.column_type() {
        ColumnType::Sequence | ColumnType::LongSequence => " integer".to_string(),
        ColumnType::Byte => " tinyint".to_string(),
        ColumnType::Short => " smallint".to_string(),
        ColumnType::Int => " integer".to_string(),
        ColumnType::Long => " bigint".to_string(),
        ColumnType::Float => " real".to_string(),
        ColumnType::Double => " double precision".to_string(),
        ColumnType::Decimal => format!(" {}", decimal_sql(column)),
        ColumnType::Boolean => format!(" {}", boolean_sql(database_model.boolean_mode())),
        ColumnType::Date | ColumnType::DateTime | ColumnType::Time | ColumnType::Timestamp => " text".to_string(),
        // Not a typo: SQLite's real generator doesn't override `timestamp_tz_sql`, so it
        // falls through to the shared default ("timestamp"), unlike the other temporal
        // types (which it does override to "text").
        ColumnType::TimestampTz => " timestamp".to_string(),
        ColumnType::Char => format!(" char({})", column.length()),
        ColumnType::Varchar => format!(" varchar({})", column.length()),
        ColumnType::Text | ColumnType::CiText | ColumnType::CsText => " text".to_string(),
        ColumnType::Enum => format!(" {}", enum_sql(database_model, column)),
        ColumnType::Binary => " blob".to_string(),
        ColumnType::Uuid | ColumnType::Json => " text".to_string(),
        ColumnType::Array => " text".to_string(),
    }
}

fn boolean_sql(boolean_mode: BooleanMode) -> String {
    match boolean_mode {
        BooleanMode::YesNo => "varchar(3)".to_string(),
        BooleanMode::YN => "char(1)".to_string(),
        BooleanMode::Native => "boolean".to_string(),
    }
}

fn decimal_sql(column: &Column) -> String {
    let length = column.length();
    let scale = column.scale();
    if length == 0 && scale == 0 {
        "decimal".to_string()
    } else if scale == 0 {
        format!("decimal({})", length)
    } else {
        format!("decimal({},{})", length, scale)
    }
}

fn enum_sql(database_model: &DatabaseModel, column: &Column) -> String {
    let enum_type_name = column.enum_type().expect("enum column is missing its enum_type");
    let enum_type = database_model.find_enum_type(column.schema_name(), enum_type_name);

    let mut min_length = usize::MAX;
    let mut max_length = 0;

    for value in enum_type.values() {
        let code = value.code();
        min_length = min_length.min(code.len());
        max_length = max_length.max(code.len());
    }

    if min_length != max_length {
        format!("varchar({})", max_length)
    } else {
        format!("char({})", max_length)
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
                "CREATE UNIQUE INDEX IF NOT EXISTS {} ON {} ({});",
                constraint_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Sqlite, table_name, ordinal);
            writeln!(
                writer,
                "CREATE INDEX IF NOT EXISTS {} ON {} ({});",
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
            writeln!(writer, "DROP INDEX IF EXISTS {};", constraint_name)?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Sqlite, table_name, ordinal);
            writeln!(writer, "DROP INDEX IF EXISTS {};", idx_name)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation) -> Result<(), MigrationGeneratorError> {
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " ON DELETE CASCADE",
        RelationType::SetNull => " ON DELETE SET NULL",
        RelationType::DoNothing => " ON DELETE RESTRICT",
        RelationType::Enforce => "",
    };
    writeln!(
        writer,
        "-- SQLite foreign keys must be declared at table creation time."
    )?;
    writeln!(
        writer,
        "-- Ensure FOREIGN KEY ({}) REFERENCES {}({}){} is in the CREATE TABLE statement for '{}'.",
        relation.from_column_name(),
        relation.to_table_name(),
        relation.to_column_name(),
        on_delete,
        relation.from_table_name()
    )?;
    writeln!(writer)?;
    Ok(())
}
