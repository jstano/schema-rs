use std::cmp;
use std::io::Write;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::types::{BooleanMode, DatabaseType, KeyType, RelationType};
use schema_model::naming::{foreign_key_name, index_name, primary_key_name, unique_key_name};

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
                    let type_sql = column_type_sql(database_model, column);
                    let not_null = if column.required() { " NOT NULL" } else { " NULL" };
                    let default = default_sql(database_model, column)
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

                    if let Some(expr) = check_constraint::check_expr(database_model, column) {
                        let name = check_constraint::constraint_name(table_name, column.name());
                        writeln!(
                            writer,
                            "ALTER TABLE {} ADD CONSTRAINT {} CHECK ({});",
                            table_name, name, expr
                        )?;
                        writeln!(writer, "GO")?;
                        writeln!(writer)?;
                    }
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
                    let type_sql = column_type_sql(database_model, new_column);
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
                SchemaChange::AddKey { table_name, key, ordinal } => {
                    write_add_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::DropKey { table_name, key, ordinal } => {
                    write_drop_key(writer, table_name, key, *ordinal)?;
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
                SchemaChange::AddRelation { relation, ordinal } => {
                    write_add_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::DropRelation { relation, ordinal } => {
                    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), *ordinal);
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

fn column_type_sql(database_model: &DatabaseModel, column: &Column) -> String {
    match column.column_type() {
        ColumnType::Sequence => " integer identity(1,1)".to_string(),
        ColumnType::LongSequence => " bigint identity(1,1)".to_string(),
        ColumnType::Byte => " tinyint".to_string(),
        ColumnType::Short => " smallint".to_string(),
        ColumnType::Int => " integer".to_string(),
        ColumnType::Long => " bigint".to_string(),
        ColumnType::Float => " real".to_string(),
        ColumnType::Double => " double precision".to_string(),
        ColumnType::Decimal => format!(" {}", decimal_sql(column)),
        ColumnType::Boolean => format!(" {}", boolean_sql(database_model.boolean_mode())),
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
        ColumnType::Text | ColumnType::CiText | ColumnType::CsText => {
            if column.length() > 0 {
                format!(" nvarchar({})", column.length())
            } else {
                " nvarchar(max)".to_string()
            }
        }
        ColumnType::Enum => format!(" {}", enum_sql(database_model, column)),
        ColumnType::Binary => {
            if column.length() > 0 {
                format!(" varbinary({})", column.length())
            } else {
                " varbinary(max)".to_string()
            }
        }
        ColumnType::Uuid => " uniqueidentifier".to_string(),
        ColumnType::Json => " json".to_string(),
        ColumnType::Array => " nvarchar(max)".to_string(),
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

fn boolean_sql(boolean_mode: BooleanMode) -> String {
    match boolean_mode {
        BooleanMode::YesNo => "nvarchar(3)".to_string(),
        BooleanMode::YN => "nchar(1)".to_string(),
        BooleanMode::Native => "bit".to_string(),
    }
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

fn enum_sql(database_model: &DatabaseModel, column: &Column) -> String {
    let enum_type_name = column.enum_type().expect("enum column is missing its enum_type");
    let enum_type = database_model.find_enum_type(column.schema_name(), enum_type_name);

    let mut min_length = usize::MAX;
    let mut max_length = 0;

    enum_type.values().iter().for_each(|enum_value| {
        let code = enum_value.code();
        min_length = cmp::min(min_length, code.len());
        max_length = cmp::max(max_length, code.len());
    });

    if min_length != max_length {
        return format!("nvarchar({})", max_length);
    }

    format!("nchar({})", max_length)
}

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let cols: String = key.columns().iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
    match key.key_type() {
        KeyType::Primary => {
            writeln!(writer, "ALTER TABLE {} ADD PRIMARY KEY ({});", table_name, cols)?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::SqlServer, table_name, ordinal);
            writeln!(
                writer,
                "CREATE UNIQUE INDEX {} ON {} ({});",
                constraint_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::SqlServer, table_name, ordinal);
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

fn write_drop_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    match key.key_type() {
        KeyType::Primary => {
            let constraint_name = primary_key_name(DatabaseType::SqlServer, table_name);
            writeln!(
                writer,
                "ALTER TABLE {} DROP CONSTRAINT {};",
                table_name, constraint_name
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::SqlServer, table_name, ordinal);
            writeln!(
                writer,
                "IF EXISTS (SELECT 1 FROM sys.indexes WHERE name = '{}') DROP INDEX {} ON {};",
                constraint_name, constraint_name, table_name
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::SqlServer, table_name, ordinal);
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

fn write_add_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), ordinal);
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
