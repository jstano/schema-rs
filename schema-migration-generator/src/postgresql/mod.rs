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
use schema_sql_generator::postgresql::postgres_column_type_generator::PostgresColumnTypeGenerator;

use crate::check_constraint;
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
        let context = GeneratorContext::for_model(Rc::new(database_model.clone()), DatabaseType::Postgresql);
        let type_generator = PostgresColumnTypeGenerator::new(context.clone());
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    writeln!(writer, "create table {} ();", table_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::DropTable { table_name } => {
                    writeln!(writer, "drop table if exists {};", table_name)?;
                    writeln!(writer)?;
                }
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
                    writeln!(
                        writer,
                        "alter table {} add column {}{}{}{};",
                        table_name,
                        column.name(),
                        type_sql,
                        not_null,
                        default
                    )?;
                    writeln!(writer)?;

                    // Enums are excluded: Postgres represents them as a native enum type
                    // (see `PostgresColumnTypeGenerator::enum_sql`), so the value list is
                    // already enforced by the type itself - unlike the other databases, which
                    // emulate enums with a plain string column plus a CHECK constraint (see
                    // `PostgresColumnConstraintGenerator`'s matching exclusion).
                    if column.column_type() != ColumnType::Enum
                        && let Some(check_sql) = check_constraint::check_constraint_sql(&context, column)
                    {
                        let name = check_constraint::constraint_name(table_name, column.name());
                        writeln!(
                            writer,
                            "alter table {} add constraint {} {};",
                            table_name, name, check_sql
                        )?;
                        writeln!(writer)?;
                    }
                }
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   alter table {} rename column {} to {};", table_name, column_name, candidate)?;
                        }
                    }
                    writeln!(writer, "alter table {} drop column {};", table_name, column_name)?;
                    writeln!(writer)?;
                }
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    writeln!(
                        writer,
                        "alter table {} rename column {} to {};",
                        table_name, old_name, new_name
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::ModifyColumn { table_name, old_column, new_column } => {
                    let new_type = format!(" {}", type_generator.column_type_sql(&dummy_table, new_column));
                    let old_type = format!(" {}", type_generator.column_type_sql(&dummy_table, old_column));
                    if new_type != old_type {
                        writeln!(
                            writer,
                            "alter table {} alter column {} type {};",
                            table_name,
                            new_column.name(),
                            new_type
                        )?;
                    }
                    if old_column.required() != new_column.required() {
                        if new_column.required() {
                            writeln!(
                                writer,
                                "alter table {} alter column {} set not null;",
                                table_name,
                                new_column.name()
                            )?;
                        } else {
                            writeln!(
                                writer,
                                "alter table {} alter column {} drop not null;",
                                table_name,
                                new_column.name()
                            )?;
                        }
                    }
                    if old_column.default_constraint() != new_column.default_constraint() {
                        if let Some(default) = default_sql(database_model, new_column) {
                            writeln!(
                                writer,
                                "alter table {} alter column {} set default {};",
                                table_name,
                                new_column.name(),
                                default
                            )?;
                        } else {
                            writeln!(
                                writer,
                                "alter table {} alter column {} drop default;",
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
                        "alter table {} add constraint {} check ({});",
                        table_name,
                        constraint.name(),
                        constraint.sql()
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    writeln!(
                        writer,
                        "alter table {} drop constraint {};",
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
                        "alter table {} drop constraint {};",
                        relation.from_table_name(),
                        fk_name
                    )?;
                    writeln!(writer)?;
                }
                SchemaChange::AddView { view } => {
                    writeln!(writer, "create or replace view {} as", view.name())?;
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
            writeln!(writer, "alter table {} add primary key ({});", table_name, cols)?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(
                writer,
                "create unique index {} on {} ({});",
                constraint_name, table_name, cols
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(
                writer,
                "create index {} on {} ({});",
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
                "alter table {} drop constraint {};",
                table_name, constraint_name
            )?;
        }
        KeyType::Unique => {
            let constraint_name = unique_key_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(writer, "drop index if exists {};", constraint_name)?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::Postgresql, table_name, ordinal);
            writeln!(writer, "drop index if exists {};", idx_name)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_add_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::Postgresql, relation.from_table_name(), ordinal);
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " on delete cascade",
        RelationType::SetNull => " on delete set null",
        RelationType::DoNothing => " on delete restrict",
        RelationType::Enforce => "",
    };
    writeln!(
        writer,
        "alter table {} add constraint {} foreign key ({}) references {}({}){};",
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
