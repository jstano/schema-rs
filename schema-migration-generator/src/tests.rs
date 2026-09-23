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

use crate::create_generator;

fn default_model() -> DatabaseModel {
    let schema = SchemaBuilder::new(None::<&str>).build();
    DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
}

#[test]
fn postgresql_add_table() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddTable { table_name: "users".to_string() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("create table if not exists users"));
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
        column: ColumnBuilder::new(Some("s"), "email", ColumnType::Varchar).required(true).build(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("alter table users add column if not exists email"));
    assert!(sql.contains("not null"));
}

#[test]
fn sqlserver_uses_go_separator() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddTable { table_name: "items".to_string() });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("go"));
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
        column: ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
            .enum_type(Some("status_type".to_string()))
            .required(true)
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
fn sqlite_rename_table() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::RenameTable {
        old_name: "old_users".to_string(),
        new_name: "users".to_string(),
    });

    let generator = create_generator(DatabaseType::Sqlite);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("alter table old_users rename to users"));
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
        sql.contains(&format!("drop index if exists {}", expected_name)),
        "expected drop index if exists {} in:\n{}", expected_name, sql
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
fn postgresql_add_unique_key_uses_if_not_exists() {
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
    assert!(sql.contains("create unique index if not exists"), "got: {}", sql);
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
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "total >= 0", DatabaseType::Postgresql),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("do $$"), "expected a guarded DO block, got: {}", sql);
    assert!(sql.contains("conname = 'ck_orders_total'"), "got: {}", sql);
    assert!(sql.contains("add constraint ck_orders_total check (total >= 0)"), "got: {}", sql);
}

#[test]
fn postgresql_drop_constraint_uses_if_exists() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_total".to_string(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("drop constraint if exists ck_orders_total"), "got: {}", sql);
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
fn postgresql_rename_column_is_guarded_by_information_schema_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::RenameColumn {
        table_name: "users".to_string(),
        old_name: "first_name".to_string(),
        new_name: "given_name".to_string(),
    });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("do $$"), "expected a guarded DO block, got: {}", sql);
    assert!(sql.contains("from information_schema.columns where table_name = 'users' and column_name = 'first_name'"), "got: {}", sql);
    assert!(sql.contains("rename column first_name to given_name"), "got: {}", sql);
}

#[test]
fn sqlserver_add_table_is_guarded_by_object_id_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddTable { table_name: "items".to_string() });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("if object_id('items', 'U') is null begin"), "got: {}", sql);
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
fn sqlserver_add_unique_key_is_guarded_by_sys_indexes_check() {
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
    assert!(sql.contains("not exists (select 1 from sys.indexes where name ="), "got: {}", sql);
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
fn sqlserver_add_constraint_is_guarded_by_sys_check_constraints_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddConstraint {
        table_name: "orders".to_string(),
        constraint: schema_model::model::constraint::Constraint::new("ck_orders_total", "total >= 0", DatabaseType::SqlServer),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("from sys.check_constraints where name = 'ck_orders_total'"), "got: {}", sql);
}

#[test]
fn sqlserver_drop_constraint_is_guarded_by_sys_objects_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropConstraint {
        table_name: "orders".to_string(),
        constraint_name: "ck_orders_total".to_string(),
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
fn sqlserver_rename_table_is_guarded_by_object_id_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::RenameTable {
        old_name: "orders".to_string(),
        new_name: "sales_orders".to_string(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(
        sql.contains("if object_id('orders', 'U') is not null and object_id('sales_orders', 'U') is null begin"),
        "got: {}", sql
    );
}

#[test]
fn sqlserver_rename_column_is_guarded_by_sys_columns_check() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::RenameColumn {
        table_name: "users".to_string(),
        old_name: "first_name".to_string(),
        new_name: "given_name".to_string(),
    });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("name = 'first_name') and not exists"), "got: {}", sql);
    assert!(sql.contains("sp_rename 'users.first_name', 'given_name', 'COLUMN'"), "got: {}", sql);
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
