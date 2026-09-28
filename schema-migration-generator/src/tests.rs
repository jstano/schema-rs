use schema_diff::SchemaDiffEngine;
use schema_diff::change::SchemaChange;
use schema_diff::change_set::ChangeSet;
use schema_model::builder::column::ColumnBuilder;
use schema_model::builder::{SchemaBuilder, TableBuilder};
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::enum_type::{EnumType, EnumValue};
use schema_model::model::key::{Key, KeyColumn};
use schema_model::model::relation::Relation;
use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, KeyType, RelationType};
use schema_model::naming::{foreign_key_name, index_name, primary_key_name, unique_key_name};

use crate::{create_generator, create_generator_with_versions};

fn default_model() -> DatabaseModel {
    let schema = SchemaBuilder::new(None::<&str>).build();
    DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
}

#[test]
fn postgresql_add_table() {
    let mut cs = ChangeSet::new();
    let table = TableBuilder::new(None::<&str>, "users")
        .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Int).required(true).build())
        .add_column(ColumnBuilder::new(None::<&str>, "email", ColumnType::Varchar).required(true).build())
        .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
        .build();
    cs.add_change(SchemaChange::AddTable { table: table.clone() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    // C7: `AddTable` must render the real table - not the old always-empty `create table if
    // not exists users ();` stub.
    assert!(sql.contains("create table if not exists public.users"), "got: {}", sql);
    assert!(sql.contains("id integer not null"), "expected the real 'id' column, got: {}", sql);
    assert!(sql.contains("email text not null"), "expected the real 'email' column, got: {}", sql);
    assert!(sql.contains("primary key"), "expected the primary key constraint, got: {}", sql);
}

/// End-to-end regression test for C7: unlike every other test in this file, which hand-builds
/// a `ChangeSet` directly, this one runs the real `SchemaDiffEngine::diff` (old schema -> new
/// schema with a brand-new table that has a column, a key, an index, and a foreign key to an
/// already-existing table) through the real `PostgresqlMigrationGenerator` - the BUGS doc's own
/// root-cause note is that hand-built `ChangeSet`s let C7 ship unnoticed, since they never
/// exercise `diff_add_tables`/`diff_add_relations` together.
#[test]
fn postgresql_end_to_end_diff_and_generate_for_a_brand_new_table_with_a_relation() {
    let customers = || {
        TableBuilder::new(Some("s"), "customers")
            .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build()
    };
    let old = SchemaBuilder::new(Some("s")).add_table(customers()).build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(customers())
        .add_table(
            TableBuilder::new(Some("s"), "orders")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "customer_id", ColumnType::Int).required(true).build())
                .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
                .add_relation(Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false))
                .build(),
        )
        .build();

    let change_set = SchemaDiffEngine::diff(&old, &new);
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![new]);

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&change_set, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    assert!(sql.contains("create table if not exists s.orders"), "got: {}", sql);
    assert!(sql.contains("customer_id integer not null"), "expected the real column, got: {}", sql);
    // The relation to the pre-existing "customers" table must still come through as a
    // separate `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY` - it isn't part of the
    // `CREATE TABLE` body under `ForeignKeyMode::Relations`.
    assert!(sql.contains("foreign key (customer_id) references customers(id)"), "got: {}", sql);
}

#[test]
fn postgresql_drop_table() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropTable { table_name: "orders".to_string() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("drop table if exists orders"));
}

#[test]
fn postgresql_add_column() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        // A default is required for the inline `not null` path exercised here - a required
        // column with no default takes the backfill-guard path instead (see M25's dedicated
        // tests below).
        column: ColumnBuilder::new(Some("s"), "email", ColumnType::Varchar)
            .required(true)
            .default_constraint(Some("'unknown'".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("alter table users add column if not exists email"));
    assert!(sql.contains("not null"));
}

#[test]
fn postgresql_add_not_null_column_with_no_default_gets_backfill_guard() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(Some("s"), "email", ColumnType::Varchar).required(true).build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    // Added nullable first - `not null` must not appear on this statement, or it would fail
    // outright against a table with existing rows.
    let add_pos = sql.find("alter table users add column if not exists email").unwrap();
    let add_line_end = sql[add_pos..].find(';').unwrap() + add_pos;
    assert!(!sql[add_pos..add_line_end].contains("not null"), "column must be added nullable, got:\n{}", sql);

    assert!(sql.contains("-- WARNING: 'email' is being made NOT NULL"), "expected a backfill warning, got:\n{}", sql);
    assert!(sql.contains("raise exception"), "expected an unconditional halt, got:\n{}", sql);
    assert!(sql.contains("alter table users alter column email set not null;"), "expected a trailing SET NOT NULL, got:\n{}", sql);

    let raise_pos = sql.find("raise exception").unwrap();
    let set_not_null_pos = sql.find("alter column email set not null").unwrap();
    assert!(raise_pos < set_not_null_pos, "expected the halt before the final SET NOT NULL, got:\n{}", sql);
}

#[test]
fn sqlserver_uses_go_separator() {
    let mut cs = ChangeSet::new();
    let table = TableBuilder::new(None::<&str>, "items")
        .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Int).required(true).build())
        .build();
    cs.add_change(SchemaChange::AddTable { table });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("go"));
}

/// End-to-end regression test for C7, SQL Server side - see the matching Postgres test's doc
/// comment. SQL Server has no `create table if not exists`, so a brand-new table's `CREATE
/// TABLE` must come out guarded by an `object_id(...) is null` check, same as every other
/// SQL Server DDL statement here, rather than the old hand-rolled `create table {} ();` (a hard
/// T-SQL syntax error on a zero-column table).
#[test]
fn sqlserver_end_to_end_diff_and_generate_for_a_brand_new_table_with_a_relation() {
    let customers = || {
        TableBuilder::new(Some("s"), "customers")
            .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build()
    };
    let old = SchemaBuilder::new(Some("s")).add_table(customers()).build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(customers())
        .add_table(
            TableBuilder::new(Some("s"), "orders")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "customer_id", ColumnType::Int).required(true).build())
                .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
                .add_index(Key::new(KeyType::Index, vec![KeyColumn::new("customer_id")]))
                .add_relation(Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false))
                .build(),
        )
        .build();

    let change_set = SchemaDiffEngine::diff(&old, &new);
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![new]);

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&change_set, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    assert!(sql.contains("if object_id('s.orders', 'U') is null begin"), "got: {}", sql);
    assert!(sql.contains("customer_id integer not null"), "expected the real column, got: {}", sql);
    assert!(sql.contains("foreign key (customer_id) references customers(id)"), "got: {}", sql);

    // Regression check found while implementing this fix: `output_indexes` renders each
    // `create index` already terminated with its own `go` (correct for the full-schema
    // generator, where every statement is its own top-level batch) - embedding that directly
    // inside the `begin ... end` this table's own guard wraps the `CREATE TABLE` in would end
    // the batch early, leaving a dangling `end` with no matching `begin`. The index must come
    // out as its own separate, top-level guarded statement instead.
    let create_table_begin = sql.find("if object_id('s.orders', 'U') is null begin").unwrap();
    let create_table_end = sql[create_table_begin..].find("\nend\ngo").unwrap() + create_table_begin;
    assert!(!sql[create_table_begin..create_table_end].contains("create index"), "got: {}", sql);
    assert!(sql.contains("create index ix_orders1 on s.orders (customer_id);"), "got: {}", sql);
}

#[test]
fn sqlserver_add_column_text_with_length_uses_bounded_nvarchar() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(Some("s"), "location_name", ColumnType::Text)
            .length(200)
            .required(true)
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("nvarchar(200)"), "expected bounded nvarchar, got: {}", sql);
    assert!(!sql.contains("nvarchar(max)"), "expected bounded nvarchar, got: {}", sql);
}

// M6: `Json` uses `nvarchar(max)` by default (SQL Server's native `json` type doesn't exist
// before SQL Server 2025), and only switches to `json` when the migration is explicitly
// generated with `--sqlserver-version 2025` or later.
#[test]
fn sqlserver_add_column_json_defaults_to_nvarchar_max() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "settings".to_string(),
        column: ColumnBuilder::new(Some("s"), "payload", ColumnType::Json).build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("nvarchar(max)"), "got: {}", sql);
}

#[test]
fn sqlserver_add_column_json_uses_native_json_when_targeting_sql_server_2025() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "settings".to_string(),
        column: ColumnBuilder::new(Some("s"), "payload", ColumnType::Json).build(),
    });

    let generator = create_generator_with_versions(DatabaseType::SqlServer, 0, 2025);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains(" json"), "got: {}", sql);
    assert!(!sql.contains("nvarchar(max)"), "got: {}", sql);
}

#[test]
fn sqlserver_add_column_boolean_respects_boolean_mode() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(Some("s"), "active", ColumnType::Boolean).required(true).build(),
    });

    let schema = SchemaBuilder::new(None::<&str>).build();
    let model = DatabaseModel::new(BooleanMode::YN, ForeignKeyMode::Relations, vec![schema]);

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("nchar(1)"), "expected nchar(1) for YN boolean mode, got: {}", sql);
}

#[test]
fn sqlserver_add_column_enum_sizes_from_enum_values() {
    use schema_model::model::enum_type::{EnumType, EnumValue};

    let enum_type = EnumType::new(
        "gender_type",
        vec![EnumValue::new("MALE", Some("M".to_string())), EnumValue::new("FEMALE", Some("F".to_string()))],
    );
    let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "accounts".to_string(),
        column: ColumnBuilder::new(None::<&str>, "gender", ColumnType::Enum)
            .enum_type(Some("gender_type".to_string()))
            .required(true)
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("nchar(1)"), "expected nchar(1) sized from enum codes, got: {}", sql);
}

#[test]
fn sqlserver_add_column_enum_emits_check_constraint_for_allowed_codes() {
    use schema_model::model::enum_type::{EnumType, EnumValue};

    let enum_type = EnumType::new(
        "status_type",
        vec![EnumValue::new("ACTIVE", Some("A".to_string())), EnumValue::new("INACTIVE", Some("I".to_string()))],
    );
    let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
            .enum_type(Some("status_type".to_string()))
            .required(true)
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("add constraint ck_location_status_") && sql.contains("check(status in ('A','I'))"),
        "expected a CHECK constraint restricting status to the enum's codes, got: {}",
        sql
    );
}

#[test]
fn sqlite_add_column_enum_inlines_check_constraint() {
    use schema_model::model::enum_type::{EnumType, EnumValue};

    let enum_type = EnumType::new(
        "status_type",
        vec![EnumValue::new("ACTIVE", Some("A".to_string())), EnumValue::new("INACTIVE", Some("I".to_string()))],
    );
    let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        // SQLite's `ADD COLUMN` rejects `NOT NULL` without a non-null `DEFAULT` (M25), so a
        // default is required here to exercise the `not null` + inline `CHECK` rendering this
        // test is actually about.
        column: ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
            .enum_type(Some("status_type".to_string()))
            .required(true)
            .default_constraint(Some("A".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("not null check(status in ('A','I'))"),
        "expected an inline CHECK constraint restricting status to the enum's codes, got: {}",
        sql
    );
}

#[test]
fn postgresql_add_column_enum_does_not_emit_check_constraint() {
    use schema_model::model::enum_type::{EnumType, EnumValue};

    // Postgres represents the enum as a native type, so the value list is already enforced
    // by the type itself and no CHECK constraint should be added (unlike Sqlite/SqlServer).
    let enum_type = EnumType::new(
        "status_type",
        vec![EnumValue::new("ACTIVE", Some("A".to_string())), EnumValue::new("INACTIVE", Some("I".to_string()))],
    );
    let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
            .enum_type(Some("status_type".to_string()))
            .required(true)
            .build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(!sql.to_lowercase().contains("check"), "expected no CHECK constraint, got: {}", sql);
}

#[test]
fn sqlserver_add_column_min_max_emits_check_constraint() {
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![SchemaBuilder::new(None::<&str>).build()]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "product".to_string(),
        column: ColumnBuilder::new(None::<&str>, "price", ColumnType::Int)
            .min_value(Some(0.0))
            .max_value(Some(100.0))
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("check(price >= 0 and price <= 100)"),
        "expected a min/max CHECK constraint, got: {}",
        sql
    );
}

#[test]
fn sqlserver_add_not_null_column_with_no_default_gets_backfill_guard() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(None::<&str>, "email", ColumnType::Varchar).length(100).required(true).build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    // Added nullable first - `not null` must not appear on this statement, or it would fail
    // outright against a table with existing rows.
    let add_pos = sql.find("alter table users add email").unwrap();
    let add_line_end = sql[add_pos..].find(';').unwrap() + add_pos;
    assert!(sql[add_pos..add_line_end].contains(" null"), "column must still be added, got:\n{}", sql);
    assert!(!sql[add_pos..add_line_end].contains("not null"), "column must be added nullable, got:\n{}", sql);

    assert!(sql.contains("-- WARNING: 'email' is being made NOT NULL"), "expected a backfill warning, got:\n{}", sql);
    assert!(sql.contains("throw 50000"), "expected an unconditional halt, got:\n{}", sql);
    assert!(sql.contains("alter table users alter column email"), "expected a trailing ALTER COLUMN ... NOT NULL, got:\n{}", sql);
    assert!(sql.contains("not null;"), "expected the trailing ALTER COLUMN to set NOT NULL, got:\n{}", sql);

    let throw_pos = sql.find("throw 50000").unwrap();
    let set_not_null_pos = sql.rfind("alter column email").unwrap();
    assert!(throw_pos < set_not_null_pos, "expected the halt before the final NOT NULL, got:\n{}", sql);
}

#[test]
fn sqlite_add_not_null_column_with_no_default_omits_not_null() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(None::<&str>, "email", ColumnType::Varchar).required(true).build(),
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    // SQLite's ADD COLUMN rejects NOT NULL with no non-null DEFAULT outright - even against an
    // empty table - so this must never be emitted at all here.
    assert!(!sql.contains("not null"), "SQLite ADD COLUMN must not emit NOT NULL without a default, got:\n{}", sql);
    assert!(sql.contains("alter table users add column email"), "expected the column to still be added, got:\n{}", sql);
    assert!(sql.contains("-- WARNING: 'email' should be NOT NULL"), "expected a manual-rebuild warning, got:\n{}", sql);
}

#[test]
fn postgresql_add_column_text_respects_case_sensitive_text() {
    let schema = SchemaBuilder::new(None::<&str>).case_sensitive_text(false).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(None::<&str>, "notes", ColumnType::Text).build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("add column if not exists notes citext"), "expected citext for case-insensitive schema, got: {}", sql);
}

#[test]
fn postgresql_add_column_enum_uses_native_enum_type_name() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "accounts".to_string(),
        column: ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
            .enum_type(Some("StatusType".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("add column if not exists status status_type"), "expected native enum type, got: {}", sql);
}

#[test]
fn postgresql_add_column_array_uses_element_type() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "widgets".to_string(),
        column: ColumnBuilder::new(None::<&str>, "tags", ColumnType::Array)
            .element_type(Some("int".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("add column if not exists tags integer[]"), "expected integer[] from elementType, got: {}", sql);
}

#[test]
fn postgresql_add_column_boolean_respects_boolean_mode() {
    let schema = SchemaBuilder::new(None::<&str>).build();
    let model = DatabaseModel::new(BooleanMode::YesNo, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean).build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("add column if not exists active varchar(3)"), "expected varchar(3) for YesNo boolean mode, got: {}", sql);
}

#[test]
fn sqlserver_add_column_boolean_default_converts_to_boolean_mode_literal() {
    let schema = SchemaBuilder::new(None::<&str>).build();
    let model = DatabaseModel::new(BooleanMode::YesNo, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean)
            .required(true)
            .default_constraint(Some("false".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("default 'No'"), "expected 'No' literal for YesNo mode, got: {}", sql);
    assert!(!sql.contains("default false"), "raw XML value leaked through unconverted: {}", sql);
}

#[test]
fn sqlserver_add_column_boolean_default_native_uses_bit_literal() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean)
            .required(true)
            .default_constraint(Some("true".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("default 1"), "expected bit literal 1 for native mode, got: {}", sql);
}

#[test]
fn postgresql_add_column_boolean_default_converts_to_boolean_mode_literal() {
    let schema = SchemaBuilder::new(None::<&str>).build();
    let model = DatabaseModel::new(BooleanMode::YN, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean)
            .required(true)
            .default_constraint(Some("true".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("default 'Y'"), "expected 'Y' literal for YN mode, got: {}", sql);
}

#[test]
fn postgresql_modify_column_boolean_default_converts_to_boolean_mode_literal() {
    let schema = SchemaBuilder::new(None::<&str>).build();
    let model = DatabaseModel::new(BooleanMode::YesNo, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "location".to_string(),
        old_column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean).required(true).build(),
        new_column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean)
            .required(true)
            .default_constraint(Some("true".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("set default 'Yes'"), "expected 'Yes' literal for YesNo mode, got: {}", sql);
}

#[test]
fn postgresql_modify_column_type_change_uses_explicit_cast() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "orders".to_string(),
        old_column: ColumnBuilder::new(None::<&str>, "quantity", ColumnType::Varchar).length(20).required(true).build(),
        new_column: ColumnBuilder::new(None::<&str>, "quantity", ColumnType::Int).required(true).build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("alter table orders alter column quantity type integer using quantity::integer;"),
        "expected explicit USING cast, got:\n{}", sql
    );
}

#[test]
fn postgresql_modify_column_sequence_type_never_emits_serial() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "orders".to_string(),
        old_column: ColumnBuilder::new(None::<&str>, "id", ColumnType::Long).required(true).build(),
        new_column: ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    let alter_line = sql.lines().find(|l| l.starts_with("alter table")).unwrap();
    assert!(!alter_line.contains("serial"), "serial/bigserial must never appear in ALTER COLUMN TYPE, got:\n{}", alter_line);
    assert!(
        sql.contains("alter table orders alter column id type integer using id::integer;"),
        "expected bare integer type, got:\n{}", sql
    );
    assert!(sql.contains("-- NOTE:"), "expected a sequence-column warning comment, got:\n{}", sql);
}

#[test]
fn sqlite_add_column_boolean_respects_boolean_mode_and_default() {
    let schema = SchemaBuilder::new(None::<&str>).build();
    let model = DatabaseModel::new(BooleanMode::YN, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "location".to_string(),
        column: ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean)
            .required(true)
            .default_constraint(Some("false".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("char(1)"), "expected char(1) column type for YN mode, got: {}", sql);
    assert!(sql.contains("default 'N'"), "expected 'N' literal for YN mode, got: {}", sql);
}

#[test]
fn postgresql_drop_column_with_rename_candidates_emits_todo() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropColumn {
        table_name: "users".to_string(),
        column_name: "first_name".to_string(),
        rename_candidates: vec!["full_name".to_string()],
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("-- TODO: possible rename?"));
    assert!(sql.contains("rename column first_name to full_name"));
    assert!(sql.contains("drop column if exists first_name"));
}

#[test]
fn postgresql_drop_column_no_candidates_no_todo() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropColumn {
        table_name: "users".to_string(),
        column_name: "legacy_field".to_string(),
        rename_candidates: vec![],
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(!sql.contains("-- TODO"));
    assert!(sql.contains("drop column if exists legacy_field"));
}

// Regression tests for H20: a migration's DROP/ADD statements must name the object
// exactly the way `schema-sql-generator`'s create path would have named it, using the
// same shared `schema_model::naming` functions on both sides.

#[test]
fn dropping_a_relation_uses_the_create_paths_positional_fk_name() {
    let customer_fk = Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false);
    let region_fk = Relation::new("regions", "id", "orders", "region_id", RelationType::SetNull, false);

    let old_table = TableBuilder::new(None::<&str>, "orders")
        .add_relation(customer_fk.clone())
        .add_relation(region_fk)
        .build();
    let new_table = TableBuilder::new(None::<&str>, "orders")
        .add_relation(customer_fk)
        .build();

    let old_schema = SchemaBuilder::new(None::<&str>).add_table(old_table).build();
    let new_schema = SchemaBuilder::new(None::<&str>).add_table(new_table).build();

    let change_set = SchemaDiffEngine::diff(&old_schema, &new_schema);
    assert!(change_set.changes().iter().any(|c| matches!(c, SchemaChange::DropRelation { ordinal, .. } if *ordinal == 2)));

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&change_set, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let expected_name = foreign_key_name(DatabaseType::Postgresql, "orders", 2);
    assert!(
        sql.contains(&format!("drop constraint if exists {}", expected_name)),
        "expected drop constraint if exists {} in:\n{}", expected_name, sql
    );
}

#[test]
fn dropping_a_unique_key_uses_the_create_paths_positional_ak_name() {
    let email_key = Key::new(KeyType::Unique, vec![KeyColumn::new("email")]);
    let username_key = Key::new(KeyType::Unique, vec![KeyColumn::new("username")]);

    let old_table = TableBuilder::new(None::<&str>, "users")
        .add_key(email_key.clone())
        .add_key(username_key)
        .build();
    let new_table = TableBuilder::new(None::<&str>, "users")
        .add_key(email_key)
        .build();

    let old_schema = SchemaBuilder::new(None::<&str>).add_table(old_table).build();
    let new_schema = SchemaBuilder::new(None::<&str>).add_table(new_table).build();

    let change_set = SchemaDiffEngine::diff(&old_schema, &new_schema);
    assert!(change_set.changes().iter().any(|c| matches!(c, SchemaChange::DropKey { ordinal, .. } if *ordinal == 2)));

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&change_set, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let expected_name = unique_key_name(DatabaseType::Postgresql, "users", 2);
    assert!(
        sql.contains(&format!("drop constraint if exists {}", expected_name)),
        "expected drop constraint if exists {} in:\n{}", expected_name, sql
    );
}

#[test]
fn dropping_an_index_uses_the_create_paths_positional_ix_name() {
    let name_index = Key::new(KeyType::Index, vec![KeyColumn::new("name")]);
    let status_index = Key::new(KeyType::Index, vec![KeyColumn::new("status")]);

    let old_table = TableBuilder::new(None::<&str>, "users")
        .add_index(name_index.clone())
        .add_index(status_index)
        .build();
    let new_table = TableBuilder::new(None::<&str>, "users")
        .add_index(name_index)
        .build();

    let old_schema = SchemaBuilder::new(None::<&str>).add_table(old_table).build();
    let new_schema = SchemaBuilder::new(None::<&str>).add_table(new_table).build();

    let change_set = SchemaDiffEngine::diff(&old_schema, &new_schema);

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&change_set, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let expected_name = index_name(DatabaseType::Postgresql, "users", 2);
    assert!(
        sql.contains(&format!("drop index if exists {}", expected_name)),
        "expected drop index if exists {} in:\n{}", expected_name, sql
    );
}

#[test]
fn dropping_a_primary_key_uses_the_create_paths_pk_name() {
    let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
    let old_table = TableBuilder::new(None::<&str>, "orders").add_key(pk).build();
    let new_table = TableBuilder::new(None::<&str>, "orders").build();

    let old_schema = SchemaBuilder::new(None::<&str>).add_table(old_table).build();
    let new_schema = SchemaBuilder::new(None::<&str>).add_table(new_table).build();

    let change_set = SchemaDiffEngine::diff(&old_schema, &new_schema);

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&change_set, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let expected_name = primary_key_name(DatabaseType::Postgresql, "orders");
    assert!(
        sql.contains(&format!("drop constraint if exists {}", expected_name)),
        "expected drop constraint if exists {} in:\n{}", expected_name, sql
    );
}

#[test]
fn sqlserver_drop_column_with_rename_candidates_emits_sp_rename_hint() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropColumn {
        table_name: "orders".to_string(),
        column_name: "old_col".to_string(),
        rename_candidates: vec!["new_col".to_string()],
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("-- TODO: possible rename?"));
    assert!(sql.contains("sp_rename 'orders.old_col', 'new_col', 'COLUMN'"));
    assert!(sql.contains("drop column old_col"));
}

#[test]
fn sqlserver_add_column_default_uses_named_constraint() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "orders".to_string(),
        column: ColumnBuilder::new(None::<&str>, "status", ColumnType::Varchar)
            .required(true)
            .default_constraint(Some("'pending'".to_string()))
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    let expected_name = crate::default_constraint::constraint_name("orders", "status");
    assert!(
        sql.contains(&format!("constraint {} default 'pending'", expected_name)),
        "expected named default constraint {} in:\n{}", expected_name, sql
    );
}

#[test]
fn sqlserver_drop_column_drops_default_constraint_first() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropColumn {
        table_name: "orders".to_string(),
        column_name: "status".to_string(),
        rename_candidates: vec![],
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    let expected_name = crate::default_constraint::constraint_name("orders", "status");
    assert!(
        sql.contains(&format!("if exists (select 1 from sys.default_constraints where name = '{}') begin", expected_name)),
        "expected guarded default constraint drop for {} in:\n{}", expected_name, sql
    );
    assert!(
        sql.contains(&format!("alter table orders drop constraint {};", expected_name)),
        "expected drop constraint {} in:\n{}", expected_name, sql
    );
    let default_drop_pos = sql.find(&format!("drop constraint {}", expected_name)).unwrap();
    let column_drop_pos = sql.find("drop column status").unwrap();
    assert!(default_drop_pos < column_drop_pos, "expected default constraint drop before column drop in:\n{}", sql);
}

#[test]
fn sqlserver_drop_column_drops_check_constraint_first() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropColumn {
        table_name: "orders".to_string(),
        column_name: "status".to_string(),
        rename_candidates: vec![],
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    let expected_name = crate::check_constraint::constraint_name("orders", "status");
    assert!(
        sql.contains(&format!("if exists (select 1 from sys.check_constraints where name = '{}') begin", expected_name)),
        "expected guarded check constraint drop for {} in:\n{}", expected_name, sql
    );
    assert!(
        sql.contains(&format!("alter table orders drop constraint {};", expected_name)),
        "expected drop constraint {} in:\n{}", expected_name, sql
    );
    let check_drop_pos = sql.find(&format!("drop constraint {}", expected_name)).unwrap();
    let column_drop_pos = sql.find("drop column status").unwrap();
    assert!(check_drop_pos < column_drop_pos, "expected check constraint drop before column drop in:\n{}", sql);
}

#[test]
fn sqlserver_modify_column_drops_old_default_and_check_constraints_before_altering() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "orders".to_string(),
        old_column: ColumnBuilder::new(None::<&str>, "quantity", ColumnType::Int)
            .required(true)
            .default_constraint(Some("0".to_string()))
            .min_value(Some(0.0))
            .build(),
        new_column: ColumnBuilder::new(None::<&str>, "quantity", ColumnType::Int).required(true).build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let default_name = crate::default_constraint::constraint_name("orders", "quantity");
    let check_name = crate::check_constraint::constraint_name("orders", "quantity");
    assert!(
        sql.contains(&format!("if exists (select 1 from sys.default_constraints where name = '{}') begin", default_name)),
        "expected guarded old default constraint drop in:\n{}", sql
    );
    assert!(
        sql.contains(&format!("if exists (select 1 from sys.check_constraints where name = '{}') begin", check_name)),
        "expected guarded old check constraint drop in:\n{}", sql
    );

    let default_drop_pos = sql.find(&format!("drop constraint {}", default_name)).unwrap();
    let alter_pos = sql.find("alter column quantity").unwrap();
    assert!(default_drop_pos < alter_pos, "expected default constraint drop before ALTER COLUMN in:\n{}", sql);
}

#[test]
fn sqlserver_modify_column_re_adds_new_default_and_check_constraints_after_altering() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "orders".to_string(),
        old_column: ColumnBuilder::new(None::<&str>, "quantity", ColumnType::Int).required(true).build(),
        new_column: ColumnBuilder::new(None::<&str>, "quantity", ColumnType::Int)
            .required(true)
            .default_constraint(Some("0".to_string()))
            .min_value(Some(0.0))
            .build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let default_name = crate::default_constraint::constraint_name("orders", "quantity");
    let check_name = crate::check_constraint::constraint_name("orders", "quantity");
    assert!(
        sql.contains(&format!("alter table orders add constraint {} default 0 for quantity;", default_name)),
        "expected new default constraint added in:\n{}", sql
    );
    assert!(sql.contains(&format!("alter table orders add constraint {} check", check_name)), "expected new check constraint added in:\n{}", sql);

    let alter_pos = sql.find("alter column quantity").unwrap();
    let default_add_pos = sql.find(&format!("add constraint {} default", default_name)).unwrap();
    assert!(alter_pos < default_add_pos, "expected ALTER COLUMN before new default constraint add in:\n{}", sql);
}

#[test]
fn sqlserver_modify_column_sequence_strips_identity_from_alter_column() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "orders".to_string(),
        old_column: ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build(),
        new_column: ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(false).build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(!sql.contains("identity("), "identity(...) must never appear in ALTER COLUMN, got:\n{}", sql);
    assert!(sql.contains("alter column id integer null;"), "expected bare integer type, got:\n{}", sql);
    assert!(sql.contains("-- NOTE:"), "expected an identity-column warning comment, got:\n{}", sql);
}

#[test]
fn sqlserver_modify_column_cycles_unique_key_and_index_around_alter_column() {
    let old_column = ColumnBuilder::new(None::<&str>, "sku", ColumnType::Varchar).length(20).required(true).build();
    let new_column = ColumnBuilder::new(None::<&str>, "sku", ColumnType::Varchar).length(40).required(true).build();

    let table = TableBuilder::new(None::<&str>, "widgets")
        .add_column(new_column.clone())
        .add_key(Key::new(KeyType::Unique, vec![KeyColumn::new("sku")]))
        .add_index(Key::new(KeyType::Index, vec![KeyColumn::new("category")]))
        .build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "widgets".to_string(),
        old_column,
        new_column,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let unique_name = unique_key_name(DatabaseType::SqlServer, "widgets", 1);
    assert!(
        sql.contains(&format!("alter table widgets drop constraint {};", unique_name)),
        "expected unique key drop in:\n{}", sql
    );
    assert!(
        sql.contains(&format!("alter table widgets add constraint {} unique (sku);", unique_name)),
        "expected unique key re-add in:\n{}", sql
    );
    assert!(!sql.contains("drop index"), "index on unaffected column 'category' should not be touched, got:\n{}", sql);

    let drop_pos = sql.find(&format!("drop constraint {}", unique_name)).unwrap();
    let alter_pos = sql.find("alter column sku").unwrap();
    let add_pos = sql.rfind(&format!("add constraint {} unique", unique_name)).unwrap();
    assert!(drop_pos < alter_pos && alter_pos < add_pos, "expected drop -> alter -> add ordering, got:\n{}", sql);
}

#[test]
fn sqlserver_modify_column_cycles_owned_foreign_key_around_alter_column() {
    let relation = Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false);
    let old_column = ColumnBuilder::new(None::<&str>, "customer_id", ColumnType::Int).required(true).build();
    let new_column = ColumnBuilder::new(None::<&str>, "customer_id", ColumnType::Int).required(false).build();

    let orders_table = TableBuilder::new(None::<&str>, "orders").add_column(new_column.clone()).add_relation(relation).build();
    let customers_table = TableBuilder::new(None::<&str>, "customers").build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(orders_table).add_table(customers_table).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "orders".to_string(),
        old_column,
        new_column,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let fk_name = foreign_key_name(DatabaseType::SqlServer, "orders", 1);
    assert!(sql.contains(&format!("alter table orders drop constraint {};", fk_name)), "expected owned FK drop in:\n{}", sql);
    assert!(sql.contains(&format!("alter table orders add constraint {} foreign key", fk_name)), "expected owned FK re-add in:\n{}", sql);
}

#[test]
fn sqlserver_modify_column_cycles_referencing_foreign_key_around_alter_column() {
    // The FK lives on `orders` (referencing `customers.id`), but the column being altered is
    // `customers.id` itself - SQL Server blocks ALTER COLUMN on the *referenced* side of a FK
    // just as much as the referencing side, so `orders`'s FK must be cycled too even though
    // this ModifyColumn's own table is `customers`.
    let relation = Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false);
    let old_column = ColumnBuilder::new(None::<&str>, "id", ColumnType::Int).required(true).build();
    let new_column = ColumnBuilder::new(None::<&str>, "id", ColumnType::Long).required(true).build();

    let orders_table = TableBuilder::new(None::<&str>, "orders").add_relation(relation).build();
    let customers_table = TableBuilder::new(None::<&str>, "customers").add_column(new_column.clone()).build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(orders_table).add_table(customers_table).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyColumn {
        table_name: "customers".to_string(),
        old_column,
        new_column,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();

    let fk_name = foreign_key_name(DatabaseType::SqlServer, "orders", 1);
    assert!(sql.contains(&format!("alter table orders drop constraint {};", fk_name)), "expected referencing FK drop in:\n{}", sql);
    assert!(sql.contains(&format!("alter table orders add constraint {} foreign key", fk_name)), "expected referencing FK re-add in:\n{}", sql);

    let drop_pos = sql.find(&format!("drop constraint {}", fk_name)).unwrap();
    let alter_pos = sql.find("alter column id").unwrap();
    assert!(drop_pos < alter_pos, "expected referencing FK drop before ALTER COLUMN on the referenced column, got:\n{}", sql);
}

// Idempotency tests: every statement schema-migration-generator emits (outside SQLite's
// documented plain-SQL limitations) must be safe to re-run against a database that already
// has the change applied.

#[test]
fn postgresql_add_primary_key_is_guarded_by_pg_constraint_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "orders".to_string(),
        key: Key::new(KeyType::Primary, vec![KeyColumn::new("id")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("do $$"), "expected a guarded DO block, got: {}", sql);
    assert!(sql.contains("from pg_constraint where conname ="), "expected a pg_constraint existence check, got: {}", sql);
    assert!(sql.contains("add constraint pk_orders primary key (id)"), "got: {}", sql);
}

#[test]
fn postgresql_add_unique_key_is_guarded_by_pg_constraint_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: Key::new(KeyType::Unique, vec![KeyColumn::new("email")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("do $$"), "expected a guarded DO block, got: {}", sql);
    assert!(sql.contains("from pg_constraint where conname ="), "expected a pg_constraint existence check, got: {}", sql);
    assert!(sql.contains("add constraint ak_users1 unique (email)"), "got: {}", sql);
}

#[test]
fn postgresql_add_index_uses_if_not_exists() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: Key::new(KeyType::Index, vec![KeyColumn::new("status")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create index if not exists"), "got: {}", sql);
}

#[test]
fn postgresql_add_constraint_is_guarded_by_pg_constraint_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "check (total >= 0)", DatabaseType::Postgresql),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("do $$"), "expected a guarded DO block, got: {}", sql);
    assert!(sql.contains("conname = 'ck_orders_total'"), "got: {}", sql);
    // `Constraint::sql()` is already the full `check (...)` clause (see
    // `DefaultTableConstraintGenerator` on the create path) - the migration generator must not
    // add a second `check (...)` wrapper around it (M26).
    assert!(sql.contains("add constraint ck_orders_total check (total >= 0)"), "got: {}", sql);
    assert!(!sql.contains("check (check ("), "constraint body must not be double-wrapped, got: {}", sql);
}

#[test]
fn postgresql_drop_constraint_uses_if_exists() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_total".to_string(),
        database_type: DatabaseType::Postgresql,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("drop constraint if exists ck_orders_total"), "got: {}", sql);
}

// M29: a `databaseType`-scoped constraint or view for a different dialect must not leak into
// this dialect's migration output.
#[test]
fn postgresql_migration_skips_constraint_and_view_scoped_to_another_dialect() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "check (total >= 0)", DatabaseType::SqlServer),
    });
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_old".to_string(),
        database_type: DatabaseType::SqlServer,
    });
    cs.add_change(SchemaChange::AddView {
        view: schema_model::model::view::View::new(
            None::<&str>,
            "v_orders",
            "select top 10 * from orders",
            Some(DatabaseType::SqlServer),
        ),
    });
    cs.add_change(SchemaChange::DropView {
        view_name: "v_old".to_string(),
        database_type: Some(DatabaseType::SqlServer),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.trim().is_empty(), "expected no output for another dialect's constraint/view, got: {}", sql);
}

#[test]
fn postgresql_add_relation_is_guarded_by_pg_constraint_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddRelation {
        relation: Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("do $$"), "expected a guarded DO block, got: {}", sql);
    assert!(sql.contains("foreign key (customer_id) references customers(id)"), "got: {}", sql);
}

#[test]
fn postgresql_add_composite_relation_lists_all_column_pairs() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddRelation {
        relation: Relation::new_composite(
            "customers",
            "orders",
            vec![("customer_id", "id"), ("region_id", "region_id")],
            RelationType::Cascade,
            false,
        )
        .unwrap(),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("foreign key (customer_id, region_id) references customers(id, region_id)"),
        "got: {}", sql
    );
}

#[test]
fn sqlserver_add_table_is_guarded_by_object_id_check() {
    let mut cs = ChangeSet::new();
    let table = TableBuilder::new(None::<&str>, "items")
        .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Int).required(true).build())
        .build();
    cs.add_change(SchemaChange::AddTable { table });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("if object_id('dbo.items', 'U') is null begin"), "got: {}", sql);
    assert!(sql.contains("id integer not null"), "expected the real 'id' column, got: {}", sql);
}

#[test]
fn sqlserver_add_column_is_guarded_by_sys_columns_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddColumn {
        table_name: "users".to_string(),
        column: ColumnBuilder::new(Some("s"), "email", ColumnType::Varchar).required(true).build(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("if not exists (select 1 from sys.columns where object_id = object_id('users') and name = 'email') begin"),
        "got: {}", sql
    );
}

#[test]
fn sqlserver_drop_column_is_guarded_by_sys_columns_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropColumn {
        table_name: "users".to_string(),
        column_name: "legacy".to_string(),
        rename_candidates: vec![],
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("if exists (select 1 from sys.columns where object_id = object_id('users') and name = 'legacy') begin"),
        "got: {}", sql
    );
}

#[test]
fn sqlserver_add_primary_key_is_guarded_by_sys_key_constraints_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "orders".to_string(),
        key: Key::new(KeyType::Primary, vec![KeyColumn::new("id")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("from sys.key_constraints where name = 'pk_orders'"), "got: {}", sql);
    assert!(sql.contains("add constraint pk_orders primary key (id)"), "got: {}", sql);
}

#[test]
fn sqlserver_add_unique_key_is_guarded_by_sys_key_constraints_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: Key::new(KeyType::Unique, vec![KeyColumn::new("email")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("from sys.key_constraints where name = 'ak_users1'"), "got: {}", sql);
    assert!(sql.contains("add constraint ak_users1 unique (email)"), "got: {}", sql);
}

#[test]
fn sqlserver_drop_primary_key_is_guarded_by_sys_key_constraints_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropKey {
        table_name: "orders".to_string(),
        key: Key::new(KeyType::Primary, vec![KeyColumn::new("id")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("exists (select 1 from sys.key_constraints where name = 'pk_orders'"), "got: {}", sql);
    assert!(sql.contains("drop constraint pk_orders"), "got: {}", sql);
}

#[test]
fn sqlserver_drop_unique_key_is_guarded_by_sys_key_constraints_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropKey {
        table_name: "users".to_string(),
        key: Key::new(KeyType::Unique, vec![KeyColumn::new("email")]),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("exists (select 1 from sys.key_constraints where name = 'ak_users1'"), "got: {}", sql);
    assert!(sql.contains("drop constraint ak_users1"), "got: {}", sql);
}

#[test]
fn sqlserver_add_constraint_is_guarded_by_sys_check_constraints_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "check (total >= 0)", DatabaseType::SqlServer),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("from sys.check_constraints where name = 'ck_orders_total'"), "got: {}", sql);
    assert!(sql.contains("add constraint ck_orders_total check (total >= 0)"), "got: {}", sql);
    assert!(!sql.contains("check (check ("), "constraint body must not be double-wrapped, got: {}", sql);
}

#[test]
fn sqlserver_drop_constraint_is_guarded_by_sys_objects_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_total".to_string(),
        database_type: DatabaseType::SqlServer,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("from sys.objects where name = 'ck_orders_total' and parent_object_id = object_id('orders')"),
        "got: {}", sql
    );
}

// M29: a `databaseType`-scoped constraint or view for a different dialect must not leak into
// this dialect's migration output.
#[test]
fn sqlserver_migration_skips_constraint_and_view_scoped_to_another_dialect() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "check (total >= 0)", DatabaseType::Postgresql),
    });
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_old".to_string(),
        database_type: DatabaseType::Postgresql,
    });
    cs.add_change(SchemaChange::AddView {
        view: schema_model::model::view::View::new(
            None::<&str>,
            "v_orders",
            "select * from orders limit 10",
            Some(DatabaseType::Postgresql),
        ),
    });
    cs.add_change(SchemaChange::DropView {
        view_name: "v_old".to_string(),
        database_type: Some(DatabaseType::Postgresql),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.trim().is_empty(), "expected no output for another dialect's constraint/view, got: {}", sql);
}

#[test]
fn sqlserver_add_relation_is_guarded_by_sys_foreign_keys_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddRelation {
        relation: Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("not exists (select 1 from sys.foreign_keys where name ="), "got: {}", sql);
    assert!(sql.contains("foreign key (customer_id) references customers(id)"), "got: {}", sql);
}

#[test]
fn sqlserver_add_composite_relation_lists_all_column_pairs() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddRelation {
        relation: Relation::new_composite(
            "customers",
            "orders",
            vec![("customer_id", "id"), ("region_id", "region_id")],
            RelationType::Cascade,
            false,
        )
        .unwrap(),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("foreign key (customer_id, region_id) references customers(id, region_id)"),
        "got: {}", sql
    );
}

#[test]
fn sqlserver_drop_relation_is_guarded_by_sys_foreign_keys_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropRelation {
        relation: Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("exists (select 1 from sys.foreign_keys where name ="), "got: {}", sql);
}

#[test]
fn sqlite_add_key_and_relation_and_constraint_changes_are_already_safe_or_documented() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: Key::new(KeyType::Index, vec![KeyColumn::new("status")]),
        ordinal: 1,
    });
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "total >= 0", DatabaseType::Sqlite),
    });
    cs.add_change(SchemaChange::AddRelation {
        relation: Relation::new("customers", "id", "orders", "customer_id", RelationType::Cascade, false),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create index if not exists"), "got: {}", sql);
    assert!(sql.contains("-- SQLite does not support adding constraint"), "got: {}", sql);
    assert!(sql.contains("-- SQLite foreign keys must be declared at table creation time."), "got: {}", sql);
}

// M29: a `databaseType`-scoped constraint or view for a different dialect must not leak into
// this dialect's migration output.
#[test]
fn sqlite_migration_skips_constraint_and_view_scoped_to_another_dialect() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "check (total >= 0)", DatabaseType::Postgresql),
    });
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_old".to_string(),
        database_type: DatabaseType::Postgresql,
    });
    cs.add_change(SchemaChange::AddView {
        view: schema_model::model::view::View::new(
            None::<&str>,
            "v_orders",
            "select * from orders limit 10",
            Some(DatabaseType::Postgresql),
        ),
    });
    cs.add_change(SchemaChange::DropView {
        view_name: "v_old".to_string(),
        database_type: Some(DatabaseType::Postgresql),
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.trim().is_empty(), "expected no output for another dialect's constraint/view, got: {}", sql);
}

#[test]
fn sqlite_add_composite_relation_comment_lists_all_column_pairs() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddRelation {
        relation: Relation::new_composite(
            "customers",
            "orders",
            vec![("customer_id", "id"), ("region_id", "region_id")],
            RelationType::Cascade,
            false,
        )
        .unwrap(),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("Ensure foreign key (customer_id, region_id) references customers(id, region_id)"),
        "got: {}", sql
    );
}

fn old_and_new_date_range_enum() -> (EnumType, EnumType) {
    let old = EnumType::new(
        "tip_pool_date_range_type",
        vec![EnumValue::new("DAILY", None::<String>), EnumValue::new("WEEKLY", None::<String>)],
    );
    let new = EnumType::new(
        "tip_pool_date_range_type",
        vec![
            EnumValue::new("DAILY", None::<String>),
            EnumValue::new("WEEKLY", None::<String>),
            EnumValue::new("PAY_PERIOD", None::<String>),
        ],
    );
    (old, new)
}

#[test]
fn postgresql_modify_enum_type_adds_new_value() {
    // The original repro from this session: adding PAY_PERIOD to an enum must produce a
    // real `ALTER TYPE ... ADD VALUE`, not an empty migration.
    let (old_enum_type, new_enum_type) = old_and_new_date_range_enum();
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyEnumType { old_enum_type, new_enum_type });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("alter type tip_pool_date_range_type add value if not exists 'PAY_PERIOD';"),
        "got: {}",
        sql
    );
}

#[test]
fn postgresql_modify_enum_type_warns_on_removed_value() {
    let (new_enum_type, old_enum_type) = old_and_new_date_range_enum(); // swapped: PAY_PERIOD removed
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyEnumType { old_enum_type, new_enum_type });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("-- WARNING"), "got: {}", sql);
    assert!(sql.contains("PAY_PERIOD"), "got: {}", sql);
    assert!(!sql.contains("add value"), "got: {}", sql);
    // Must not be a passive comment only - an unconditional failure forces the developer to
    // act instead of letting the migration silently succeed with a possibly-stale enum.
    assert!(sql.contains("raise exception"), "got: {}", sql);
}

#[test]
fn postgresql_add_and_drop_enum_type() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddEnumType {
        enum_type: EnumType::new("status_type", vec![EnumValue::new("ACTIVE", None::<String>)]),
    });
    cs.add_change(SchemaChange::DropEnumType { enum_type_name: "old_status_type".to_string() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create type status_type as enum ('ACTIVE');"), "got: {}", sql);
    assert!(sql.contains("drop type if exists old_status_type cascade;"), "got: {}", sql);
}

#[test]
fn sqlserver_modify_enum_type_regenerates_check_constraint_on_referencing_columns() {
    let (old_enum_type, new_enum_type) = old_and_new_date_range_enum();
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyEnumType { old_enum_type, new_enum_type: new_enum_type.clone() });

    let table = TableBuilder::new(None::<&str>, "tip_pool")
        .add_column(
            ColumnBuilder::new(None::<&str>, "date_range_type", ColumnType::Enum)
                .enum_type(Some("tip_pool_date_range_type".to_string()))
                .required(true)
                .build(),
        )
        .build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(table).add_enum_type(new_enum_type).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("alter table tip_pool drop constraint"), "got: {}", sql);
    assert!(sql.contains("alter table tip_pool add constraint"), "got: {}", sql);
    assert!(sql.contains("'PAY_PERIOD'"), "got: {}", sql);
}

#[test]
fn sqlserver_modify_enum_type_forces_failure_when_a_value_is_removed() {
    let (new_enum_type, old_enum_type) = old_and_new_date_range_enum(); // swapped: PAY_PERIOD removed
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyEnumType { old_enum_type, new_enum_type: new_enum_type.clone() });

    let table = TableBuilder::new(None::<&str>, "tip_pool")
        .add_column(
            ColumnBuilder::new(None::<&str>, "date_range_type", ColumnType::Enum)
                .enum_type(Some("tip_pool_date_range_type".to_string()))
                .required(true)
                .build(),
        )
        .build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(table).add_enum_type(new_enum_type).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    // Unconditional failure - not a passive comment - so the migration can't succeed silently
    // even when no current row happens to violate the shrunk constraint.
    assert!(sql.contains("throw 50000"), "got: {}", sql);
    assert!(sql.contains("PAY_PERIOD"), "got: {}", sql);
    // The guard must precede the drop/add constraint statements, so the script halts before
    // any of them run.
    let throw_pos = sql.find("throw 50000").unwrap();
    let drop_pos = sql.find("alter table tip_pool drop constraint").unwrap();
    assert!(throw_pos < drop_pos, "expected the throw guard before the drop constraint, got: {}", sql);
}

#[test]
fn sqlite_modify_enum_type_emits_manual_rebuild_comment() {
    let (old_enum_type, new_enum_type) = old_and_new_date_range_enum();
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyEnumType { old_enum_type, new_enum_type });

    let table = TableBuilder::new(None::<&str>, "tip_pool")
        .add_column(
            ColumnBuilder::new(None::<&str>, "date_range_type", ColumnType::Enum)
                .enum_type(Some("tip_pool_date_range_type".to_string()))
                .required(true)
                .build(),
        )
        .build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("-- SQLite does not support altering a CHECK constraint in-place"), "got: {}", sql);
    assert!(sql.contains("tip_pool.date_range_type"), "got: {}", sql);
}

#[test]
fn sqlite_modify_enum_type_emits_manual_rebuild_comment_on_value_removed() {
    let (new_enum_type, old_enum_type) = old_and_new_date_range_enum(); // swapped: PAY_PERIOD removed
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::ModifyEnumType { old_enum_type, new_enum_type });

    let table = TableBuilder::new(None::<&str>, "tip_pool")
        .add_column(
            ColumnBuilder::new(None::<&str>, "date_range_type", ColumnType::Enum)
                .enum_type(Some("tip_pool_date_range_type".to_string()))
                .required(true)
                .build(),
        )
        .build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
    let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    // SQLite can't drop a value from a CHECK constraint in-place either, so removal must
    // surface the same manual-rebuild instruction as widening - never silence.
    assert!(sql.contains("-- SQLite does not support altering a CHECK constraint in-place"), "got: {}", sql);
    assert!(sql.contains("tip_pool.date_range_type"), "got: {}", sql);
}

#[test]
fn postgresql_add_and_drop_function_and_procedure() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddFunction {
        function: schema_model::model::function::Function::new(
            None::<&str>,
            "f1",
            DatabaseType::Postgresql,
            "create function f1() returns int as $$ select 1 $$ language sql",
        ),
    });
    cs.add_change(SchemaChange::DropProcedure { procedure_name: "old_proc".to_string(), database_type: DatabaseType::Postgresql });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create function f1()"), "got: {}", sql);
    assert!(sql.contains("drop procedure if exists old_proc;"), "got: {}", sql);
}

fn unique_filtered_index_key() -> Key {
    Key::new_full_with_filter(
        KeyType::Index,
        vec![KeyColumn::new("parent_labor_structure_id")],
        false,
        false,
        true,
        None::<String>,
        Some("parent_labor_structure_id is null"),
    )
}

#[test]
fn postgresql_add_key_unique_filtered_index_renders_unique_and_where() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: unique_filtered_index_key(),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create unique index if not exists"), "got: {}", sql);
    assert!(sql.contains("where parent_labor_structure_id is null"), "got: {}", sql);
}

#[test]
fn sqlite_add_key_unique_filtered_index_renders_unique_and_where() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: unique_filtered_index_key(),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create unique index if not exists"), "got: {}", sql);
    assert!(sql.contains("where parent_labor_structure_id is null"), "got: {}", sql);
}

#[test]
fn sqlserver_add_key_unique_filtered_index_renders_unique_and_where() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddKey {
        table_name: "users".to_string(),
        key: unique_filtered_index_key(),
        ordinal: 1,
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create unique index"), "got: {}", sql);
    assert!(sql.contains("where parent_labor_structure_id is null"), "got: {}", sql);
}
