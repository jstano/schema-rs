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
use schema_sql_generator::sqlserver::sqlserver_column_type_generator::SqlServerColumnTypeGenerator;

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
        let context = GeneratorContext::for_model(Rc::new(database_model.clone()), DatabaseType::SqlServer);
        let type_generator = SqlServerColumnTypeGenerator::new(context.clone());
        let dummy_table = TableBuilder::new(None::<&str>, "_").build();

        for change in change_set.changes() {
            match change {
                SchemaChange::AddTable { table_name } => {
                    write_guarded(
                        writer,
                        &format!("object_id('{}', 'U') is null", table_name),
                        &format!("create table {} ();", table_name),
                    )?;
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
                SchemaChange::RenameTable { old_name, new_name } => {
                    write_guarded(
                        writer,
                        &format!("object_id('{}', 'U') is not null and object_id('{}', 'U') is null", old_name, new_name),
                        &format!("exec sp_rename '{}', '{}';", old_name, new_name),
                    )?;
                }
                SchemaChange::AddColumn { table_name, column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, column));
                    let not_null = if column.required() { " not null" } else { " null" };
                    let default = default_sql(database_model, column)
                        .map(|d| format!(" default {}", d))
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

                    if let Some(check_sql) = check_constraint::check_constraint_sql(&context, column) {
                        let name = check_constraint::constraint_name(table_name, column.name());
                        write_guarded(
                            writer,
                            &format!("not exists (select 1 from sys.check_constraints where name = '{}')", name),
                            &format!("alter table {} add constraint {} {};", table_name, name, check_sql),
                        )?;
                    }
                }
                SchemaChange::DropColumn { table_name, column_name, rename_candidates } => {
                    if !rename_candidates.is_empty() {
                        writeln!(writer, "-- TODO: possible rename? Consider replacing the DROP + ADD below with:")?;
                        for candidate in rename_candidates {
                            writeln!(writer, "--   exec sp_rename '{}.{}', '{}', 'COLUMN';", table_name, column_name, candidate)?;
                        }
                    }
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
                            table_name, column_name
                        ),
                        &format!("alter table {} drop column {};", table_name, column_name),
                    )?;
                }
                SchemaChange::RenameColumn { table_name, old_name, new_name } => {
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}') and not exists (select 1 from sys.columns where object_id = object_id('{}') and name = '{}')",
                            table_name, old_name, table_name, new_name
                        ),
                        &format!("exec sp_rename '{}.{}', '{}', 'COLUMN';", table_name, old_name, new_name),
                    )?;
                }
                SchemaChange::ModifyColumn { table_name, old_column: _, new_column } => {
                    let type_sql = format!(" {}", type_generator.column_type_sql(&dummy_table, new_column));
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
                }
                SchemaChange::AddKey { table_name, key, ordinal } => {
                    write_add_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::DropKey { table_name, key, ordinal } => {
                    write_drop_key(writer, table_name, key, *ordinal)?;
                }
                SchemaChange::AddConstraint { table_name, constraint } => {
                    write_guarded(
                        writer,
                        &format!("not exists (select 1 from sys.check_constraints where name = '{}')", constraint.name()),
                        &format!(
                            "alter table {} add constraint {} check ({});",
                            table_name,
                            constraint.name(),
                            constraint.sql()
                        ),
                    )?;
                }
                SchemaChange::DropConstraint { table_name, constraint_name } => {
                    write_guarded(
                        writer,
                        &format!(
                            "exists (select 1 from sys.objects where name = '{}' and parent_object_id = object_id('{}'))",
                            constraint_name, table_name
                        ),
                        &format!("alter table {} drop constraint {};", table_name, constraint_name),
                    )?;
                }
                SchemaChange::AddRelation { relation, ordinal } => {
                    write_add_relation(writer, relation, *ordinal)?;
                }
                SchemaChange::DropRelation { relation, ordinal } => {
                    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), *ordinal);
                    write_guarded(
                        writer,
                        &format!("exists (select 1 from sys.foreign_keys where name = '{}')", fk_name),
                        &format!("alter table {} drop constraint {};", relation.from_table_name(), fk_name),
                    )?;
                }
                SchemaChange::AddView { view } => {
                    writeln!(writer, "create or alter view {} as", view.name())?;
                    writeln!(writer, "{};", view.sql())?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
                SchemaChange::DropView { view_name } => {
                    writeln!(
                        writer,
                        "if object_id('{}', 'V') is not null drop view {};",
                        view_name, view_name
                    )?;
                    writeln!(writer, "go")?;
                    writeln!(writer)?;
                }
            }
        }
        Ok(())
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

/// T-SQL has no `IF [NOT] EXISTS` clause on `ALTER TABLE`/`CREATE TABLE`/`sp_rename`, so
/// idempotency has to be expressed as an explicit `IF (NOT) EXISTS (...) BEGIN ... END`
/// guard around the statement, using the relevant `sys.*` catalog view.
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
                &format!("not exists (select 1 from sys.indexes where name = '{}')", constraint_name),
                &format!("create unique index {} on {} ({});", constraint_name, table_name, cols),
            )?;
        }
        KeyType::Index => {
            let idx_name = index_name(DatabaseType::SqlServer, table_name, ordinal);
            write_guarded(
                writer,
                &format!("not exists (select 1 from sys.indexes where name = '{}')", idx_name),
                &format!("create index {} on {} ({});", idx_name, table_name, cols),
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
                "if exists (select 1 from sys.indexes where name = '{}') drop index {} on {};",
                constraint_name, constraint_name, table_name
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

fn write_add_relation(writer: &mut dyn Write, relation: &Relation, ordinal: usize) -> Result<(), MigrationGeneratorError> {
    let fk_name = foreign_key_name(DatabaseType::SqlServer, relation.from_table_name(), ordinal);
    let on_delete = match relation.relation_type() {
        RelationType::Cascade => " on delete cascade",
        RelationType::SetNull => " on delete set null",
        RelationType::DoNothing => " on delete no action",
        RelationType::Enforce => "",
    };
    write_guarded(
        writer,
        &format!("not exists (select 1 from sys.foreign_keys where name = '{}')", fk_name),
        &format!(
            "alter table {} add constraint {} foreign key ({}) references {}({}){};",
            relation.from_table_name(),
            fk_name,
            relation.from_column_name(),
            relation.to_table_name(),
            relation.to_column_name(),
            on_delete
        ),
    )?;
    Ok(())
}
