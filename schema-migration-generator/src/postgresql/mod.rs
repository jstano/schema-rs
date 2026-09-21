use std::io::Write;

use schema_diff::{ChangeSet, SchemaChange};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::types::{BooleanMode, DatabaseType, KeyType, RelationType};
use schema_model::naming::{foreign_key_name, index_name, primary_key_name, unique_key_name};

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
                    let new_type = column_type_sql(database_model, new_column);
                    let old_type = column_type_sql(database_model, old_column);
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
                        if let Some(default) = default_sql(database_model, new_column) {
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
                SchemaChange::AddRelation { relation, ordinal } => {
                    write_add_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::DropRelation { relation, ordinal } => {
                    let fk_name = foreign_key_name(DatabaseType::Postgresql, relation.from_table_name(), *ordinal);
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

fn column_type_sql(database_model: &DatabaseModel, column: &Column) -> String {
    match column.column_type() {
        ColumnType::Sequence => " serial".to_string(),
        ColumnType::LongSequence => " bigserial".to_string(),
        ColumnType::Byte => " smallint".to_string(),
        ColumnType::Short => " smallint".to_string(),
        ColumnType::Int => " integer".to_string(),
        ColumnType::Long => " bigint".to_string(),
        ColumnType::Float => " real".to_string(),
        ColumnType::Double => " double precision".to_string(),
        ColumnType::Decimal => format!(" {}", decimal_sql(column)),
        ColumnType::Boolean => format!(" {}", boolean_sql(database_model.boolean_mode())),
        ColumnType::Date => " date".to_string(),
        ColumnType::DateTime => " timestamp".to_string(),
        ColumnType::Time => " time".to_string(),
        ColumnType::Timestamp => " timestamp".to_string(),
        ColumnType::TimestampTz => " timestamptz".to_string(),
        ColumnType::Char => format!(" char({})", column.length()),
        ColumnType::Varchar => " text".to_string(),
        ColumnType::Text => format!(" {}", text_sql(database_model, column)),
        ColumnType::CiText => " citext".to_string(),
        ColumnType::CsText => " text".to_string(),
        ColumnType::Enum => format!(" {}", to_snake_case(column.enum_type().expect("enum column is missing its enum_type"))),
        ColumnType::Binary => " bytea".to_string(),
        ColumnType::Uuid => " uuid".to_string(),
        ColumnType::Json => " jsonb".to_string(),
        ColumnType::Array => format!(" {}", array_sql(database_model, column)),
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

fn text_sql(database_model: &DatabaseModel, column: &Column) -> String {
    let schema = database_model.find_schema(column.schema_name());
    if !schema.case_sensitive_text() {
        return "citext".to_string();
    }
    "text".to_string()
}

fn array_sql(database_model: &DatabaseModel, column: &Column) -> String {
    let element_type_name = column
        .element_type()
        .unwrap_or_else(|| panic!("Array column '{}' is missing an elementType.", column.name()));
    let element_type = ColumnType::from_type_name(element_type_name)
        .unwrap_or_else(|e| panic!("Array column '{}' has an invalid elementType: {}", column.name(), e));

    match element_type {
        ColumnType::Byte | ColumnType::Short => "smallint[]".to_string(),
        ColumnType::Int => "integer[]".to_string(),
        ColumnType::Long => "bigint[]".to_string(),
        ColumnType::Decimal => format!("{}[]", decimal_sql(column)),
        ColumnType::Char => format!("char({})[]", column.length()),
        ColumnType::Varchar => "text[]".to_string(),
        ColumnType::Text => format!("{}[]", text_sql(database_model, column)),
        other => panic!("Unsupported array element type: {:?}", other),
    }
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_underscore = false;

    for ch in s.chars() {
        if ch.is_uppercase() && !prev_underscore && !result.is_empty() {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
        prev_underscore = ch == '_';
    }

    result
}

fn write_add_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let cols: String = key.columns().iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
    match key.key_type() {
        KeyType::Primary => {
            writeln!(writer, "ALTER TABLE {} ADD PRIMARY KEY ({});", table_name, cols)?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(
                writer,
                "CREATE UNIQUE INDEX {} ON {} ({});",
                constraint_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
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

fn write_drop_key(writer: &mut dyn Write, table_name: &str, key: &Key, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    match key.key_type() {
        KeyType::Primary => {
            let constraint_name = primary_key_name(DatabaseType::Postgresql, table_name);
            writeln!(
                writer,
                "ALTER TABLE {} DROP CONSTRAINT {};",
                table_name, constraint_name
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(writer, "DROP INDEX IF EXISTS {};", constraint_name)?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(writer, "DROP INDEX IF EXISTS {};", idx_name)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::Postgresql, relation.from_table_name(), ordinal);
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
