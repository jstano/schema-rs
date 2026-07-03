use std::io::Write;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::types::{KeyType, RelationType};

use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

pub struct SqliteMigrationGenerator;

impl MigrationGenerator for SqliteMigrationGenerator {
    fn generate(&self, change_set: &ChangeSet, writer: &mut dyn Write) -> Result<(), MigrationGeneratorError> {
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
                    let type_sql = column_type_sql(column);
                    let not_null = if column.required() { " NOT NULL" } else { "" };
                    let default = column
                        .default_constraint()
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
                SchemaChange::AddKey { table_name, key } => {
                    write_add_key(writer, table_name, key)?;
                }
                SchemaChange::DropKey { table_name, key } => {
                    write_drop_key(writer, table_name, key)?;
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
                SchemaChange::AddRelation { relation } => {
                    write_add_relation(writer, relation)?;
                }
                SchemaChange::DropRelation { relation } => {
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

fn column_type_sql(column: &Column) -> String {
    match column.column_type() {
        ColumnType::Sequence | ColumnType::LongSequence => " INTEGER".to_string(),
        ColumnType::Byte | ColumnType::Short | ColumnType::Int | ColumnType::Long => " INTEGER".to_string(),
        ColumnType::Float | ColumnType::Double | ColumnType::Decimal => " REAL".to_string(),
        ColumnType::Boolean => " INTEGER".to_string(),
        ColumnType::Date | ColumnType::DateTime | ColumnType::Time | ColumnType::Timestamp | ColumnType::TimestampTz => {
            " TEXT".to_string()
        }
        ColumnType::Char | ColumnType::Varchar | ColumnType::Text | ColumnType::CiText | ColumnType::CsText | ColumnType::Enum => {
            " TEXT".to_string()
        }
        ColumnType::Binary => " BLOB".to_string(),
        ColumnType::Uuid | ColumnType::Json | ColumnType::Array => " TEXT".to_string(),
    }
}

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key) -> Result<(), MigrationGeneratorError> {
    let col_names: Vec<&str> = key.columns().iter().map(|c| c.name()).collect();
    let cols = col_names.join(", ");
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
            let idx_name = format!("idx_{}_{}", table_name, col_names.join("_"));
            writeln!(
                writer,
                "CREATE UNIQUE INDEX IF NOT EXISTS {} ON {} ({});",
                idx_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = format!("idx_{}_{}", table_name, col_names.join("_"));
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

fn write_drop_key(writer: &mut dyn Write, table_name: &str, key: &Key) -> Result<(), MigrationGeneratorError> {
    let col_names: Vec<&str> = key.columns().iter().map(|c| c.name()).collect();
    match key.key_type() {
        KeyType::Primary => {
            writeln!(
                writer,
                "-- SQLite does not support dropping a PRIMARY KEY constraint in-place on table '{}'.",
                table_name
            )?;
            writeln!(writer, "-- Manually recreate the table without the primary key.")?;
        }
        KeyType::Unique | KeyType::Index => {
            let idx_name = format!("idx_{}_{}", table_name, col_names.join("_"));
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
