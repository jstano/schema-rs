use std::io::Write;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::types::{KeyType, RelationType};

use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

pub struct PostgresqlMigrationGenerator;

impl MigrationGenerator for PostgresqlMigrationGenerator {
    fn generate(&self, change_set: &ChangeSet, writer: &mut dyn Write) -> Result<(), MigrationGeneratorError> {
        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    writeln!(writer, "CREATE TABLE {} ();", table_name)?;
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
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   ALTER TABLE {} RENAME COLUMN {} TO {};", table_name, column_name, candidate)?;
                        }
                    }
                    writeln!(writer, "ALTER TABLE {} DROP COLUMN {};", table_name, column_name)?;
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
                SchemaChange::ModifyColumn { table_name, old_column, new_column } => {
                    let new_type = column_type_sql(new_column);
                    let old_type = column_type_sql(old_column);
                    if new_type != old_type {
                        writeln!(
                            writer,
                            "ALTER TABLE {} ALTER COLUMN {} TYPE {};",
                            table_name,
                            new_column.name(),
                            new_type
                        )?;
                    }
                    if old_column.required() != new_column.required() {
                        if new_column.required() {
                            writeln!(
                                writer,
                                "ALTER TABLE {} ALTER COLUMN {} SET NOT NULL;",
                                table_name,
                                new_column.name()
                            )?;
                        } else {
                            writeln!(
                                writer,
                                "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL;",
                                table_name,
                                new_column.name()
                            )?;
                        }
                    }
                    if old_column.default_constraint() != new_column.default_constraint() {
                        if let Some(default) = new_column.default_constraint() {
                            writeln!(
                                writer,
                                "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
                                table_name,
                                new_column.name(),
                                default
                            )?;
                        } else {
                            writeln!(
                                writer,
                                "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                                table_name,
                                new_column.name()
                            )?;
                        }
                    }
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
                        "ALTER TABLE {} ADD CONSTRAINT {} CHECK ({});",
                        table_name,
                        constraint.name(),
                        constraint.sql()
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    writeln!(
                        writer,
                        "ALTER TABLE {} DROP CONSTRAINT {};",
                        table_name, constraint_name
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::AddRelation { relation } => {
                    write_add_relation(writer, relation)?;
                }
                SchemaChange::DropRelation { relation } => {
                    let fk_name = fk_constraint_name(relation);
                    writeln!(
                        writer,
                        "ALTER TABLE {} DROP CONSTRAINT {};",
                        relation.from_table_name(),
                        fk_name
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { view } => {
                    writeln!(writer, "CREATE OR REPLACE VIEW {} AS", view.name())?;
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
        ColumnType::Sequence => " serial".to_string(),
        ColumnType::LongSequence => " bigserial".to_string(),
        ColumnType::Byte => " smallint".to_string(),
        ColumnType::Short => " smallint".to_string(),
        ColumnType::Int => " integer".to_string(),
        ColumnType::Long => " bigint".to_string(),
        ColumnType::Float => " real".to_string(),
        ColumnType::Double => " double precision".to_string(),
        ColumnType::Decimal => {
            let l = column.length();
            let s = column.scale();
            if l == 0 && s == 0 {
                " decimal".to_string()
            } else {
                format!(" decimal({}, {})", l, s)
            }
        }
        ColumnType::Boolean => " boolean".to_string(),
        ColumnType::Date => " date".to_string(),
        ColumnType::DateTime => " timestamp".to_string(),
        ColumnType::Time => " time".to_string(),
        ColumnType::Timestamp => " timestamp".to_string(),
        ColumnType::TimestampTz => " timestamptz".to_string(),
        ColumnType::Char => format!(" char({})", column.length()),
        ColumnType::Varchar => " text".to_string(),
        ColumnType::Text => " text".to_string(),
        ColumnType::CiText => " citext".to_string(),
        ColumnType::CsText => " text".to_string(),
        ColumnType::Enum => " text".to_string(),
        ColumnType::Binary => " bytea".to_string(),
        ColumnType::Uuid => " uuid".to_string(),
        ColumnType::Json => " jsonb".to_string(),
        ColumnType::Array => " text[]".to_string(),
    }
}

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key) -> Result<(), MigrationGeneratorError> {
    let col_names: Vec<&str> = key.columns().iter().map(|c| c.name()).collect();
    let cols = col_names.join(", ");
    match key.key_type() {
        KeyType::Primary => {
            writeln!(writer, "ALTER TABLE {} ADD PRIMARY KEY ({});", table_name, cols)?;
        }
        KeyType::Unique => {
            let idx_name = format!("idx_{}_{}", table_name, col_names.join("_"));
            writeln!(
                writer,
                "CREATE UNIQUE INDEX {} ON {} ({});",
                idx_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = format!("idx_{}_{}", table_name, col_names.join("_"));
            writeln!(
                writer,
                "CREATE INDEX {} ON {} ({});",
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
                "ALTER TABLE {} DROP CONSTRAINT {}_pkey;",
                table_name, table_name
            )?;
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
    let fk_name = fk_constraint_name(relation);
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " ON DELETE CASCADE",
        RelationType::SetNull => " ON DELETE SET NULL",
        RelationType::DoNothing => " ON DELETE RESTRICT",
        RelationType::Enforce => "",
    };
    writeln!(
        writer,
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}){};",
        relation.from_table_name(),
        fk_name,
        relation.from_column_name(),
        relation.to_table_name(),
        relation.to_column_name(),
        on_delete
    )?;
    writeln!(writer)?;
    Ok(())
}

fn fk_constraint_name(relation: &Relation) -> String {
    format!(
        "fk_{}_{}",
        relation.from_table_name(),
        relation.from_column_name()
    )
}
