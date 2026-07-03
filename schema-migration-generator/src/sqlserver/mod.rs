use std::io::Write;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::types::{KeyType, RelationType};

use crate::error::MigrationGeneratorError;
use crate::migration_generator::MigrationGenerator;

pub struct SqlServerMigrationGenerator;

impl MigrationGenerator for SqlServerMigrationGenerator {
    fn generate(&self, change_set: &ChangeSet, writer: &mut dyn Write) -> Result<(), MigrationGeneratorError> {
        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    writeln!(writer, "CREATE TABLE {} ();", table_name)?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(
                        writer,
                        "IF OBJECT_ID('{}', 'U') IS NOT NULL DROP TABLE {};",
                        table_name, table_name
                    )?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameTable { old_name, new_name } => {
                    writeln!(writer, "EXEC sp_rename '{}', '{}';", old_name, new_name)?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = column_type_sql(column);
                    let not_null = if column.required() { " NOT NULL" } else { " NULL" };
                    let default = column
                        .default_constraint()
                        .map(|d| format!(" DEFAULT {}", d))
                        .unwrap_or_default();
                    writeln!(
                        writer,
                        "ALTER TABLE {} ADD {}{}{}{};",
                        table_name,
                        column.name(),
                        type_sql,
                        not_null,
                        default
                    )?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   EXEC sp_rename '{}.{}', '{}', 'COLUMN';", table_name, column_name, candidate)?;
                        }
                    }
                    writeln!(writer, "ALTER TABLE {} DROP COLUMN {};", table_name, column_name)?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    writeln!(
                        writer,
                        "EXEC sp_rename '{}.{}', '{}', 'COLUMN';",
                        table_name, old_name, new_name
                    )?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::ModifyColumn { table_name, old_column: _, new_column } => {
                    let type_sql = column_type_sql(new_column);
                    let null = if new_column.required() { " NOT NULL" } else { " NULL" };
                    writeln!(
                        writer,
                        "ALTER TABLE {} ALTER COLUMN {}{}{};",
                        table_name,
                        new_column.name(),
                        type_sql,
                        null
                    )?;
                    writeln!(writer, "GO")?;
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
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    writeln!(
                        writer,
                        "ALTER TABLE {} DROP CONSTRAINT {};",
                        table_name, constraint_name
                    )?;
                    writeln!(writer, "GO")?;
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
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { view } => {
                    writeln!(writer, "CREATE OR ALTER VIEW {} AS", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { view_name } => {
                    writeln!(
                        writer,
                        "IF OBJECT_ID('{}', 'V') IS NOT NULL DROP VIEW {};",
                        view_name, view_name
                    )?;
                    writeln!(writer, "GO")?;
                    writeln!(writer)?;
                }
            }
        }
        Ok(())
    }
}

fn column_type_sql(column: &Column) -> String {
    match column.column_type() {
        ColumnType::Sequence => " integer identity(1,1)".to_string(),
        ColumnType::LongSequence => " bigint identity(1,1)".to_string(),
        ColumnType::Byte => " smallint".to_string(),
        ColumnType::Short => " smallint".to_string(),
        ColumnType::Int => " integer".to_string(),
        ColumnType::Long => " bigint".to_string(),
        ColumnType::Float => " real".to_string(),
        ColumnType::Double => " float".to_string(),
        ColumnType::Decimal => {
            let l = column.length();
            let s = column.scale();
            if l == 0 && s == 0 {
                " decimal".to_string()
            } else {
                format!(" decimal({}, {})", l, s)
            }
        }
        ColumnType::Boolean => " bit".to_string(),
        ColumnType::Date => " datetime".to_string(),
        ColumnType::DateTime => " datetime".to_string(),
        ColumnType::Time => " datetime".to_string(),
        ColumnType::Timestamp => " datetime".to_string(),
        ColumnType::TimestampTz => " datetimeoffset".to_string(),
        ColumnType::Char => {
            let l = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };
            format!(" nchar({})", l)
        }
        ColumnType::Varchar => {
            let l = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };
            format!(" nvarchar({})", l)
        }
        ColumnType::Text | ColumnType::CiText | ColumnType::CsText | ColumnType::Enum => {
            " nvarchar(max)".to_string()
        }
        ColumnType::Binary => " varbinary(max)".to_string(),
        ColumnType::Uuid => " uniqueidentifier".to_string(),
        ColumnType::Json => " nvarchar(max)".to_string(),
        ColumnType::Array => " nvarchar(max)".to_string(),
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
    writeln!(writer, "GO")?;
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
            writeln!(
                writer,
                "IF EXISTS (SELECT 1 FROM sys.indexes WHERE name = '{}') DROP INDEX {} ON {};",
                idx_name, idx_name, table_name
            )?;
        }
    }
    writeln!(writer, "GO")?;
    writeln!(writer)?;
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation) -> Result<(), MigrationGeneratorError> {
    let fk_name = fk_constraint_name(relation);
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " ON DELETE CASCADE",
        RelationType::SetNull => " ON DELETE SET NULL",
        RelationType::DoNothing => " ON DELETE NO ACTION",
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
    writeln!(writer, "GO")?;
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
