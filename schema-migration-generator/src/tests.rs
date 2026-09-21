use schema_diff::SchemaDiffEngine;
use schema_diff::change::SchemaChange;
use schema_diff::change_set::ChangeSet;
use schema_model::builder::column::ColumnBuilder;
use schema_model::builder::{SchemaBuilder, TableBuilder};
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
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
    assert!(sql.contains("CREATE TABLE users"));
}

#[test]
fn postgresql_drop_table() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropTable { table_name: "orders".to_string() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("DROP TABLE IF EXISTS orders"));
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
    assert!(sql.contains("ALTER TABLE users ADD COLUMN email"));
    assert!(sql.contains("NOT NULL"));
}

#[test]
fn sqlserver_uses_go_separator() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddTable { table_name: "items".to_string() });

    let generator = create_generator(DatabaseType::SqlServer);
    let mut output = Vec::new();
    generator.generate(&cs, &default_model(), &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("GO"));
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
        sql.contains("ADD CONSTRAINT ck_location_status_") && sql.contains("CHECK (status in ('A','I'))"),
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
        sql.contains("NOT NULL CHECK (status in ('A','I'))"),
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
        sql.contains("CHECK (price >= 0 and price <= 100)"),
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
    assert!(sql.contains("ADD COLUMN notes citext"), "expected citext for case-insensitive schema, got: {}", sql);
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
    assert!(sql.contains("ADD COLUMN status status_type"), "expected native enum type, got: {}", sql);
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
    assert!(sql.contains("ADD COLUMN tags integer[]"), "expected integer[] from elementType, got: {}", sql);
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
    assert!(sql.contains("ADD COLUMN active varchar(3)"), "expected varchar(3) for YesNo boolean mode, got: {}", sql);
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
    assert!(sql.contains("DEFAULT 'No'"), "expected 'No' literal for YesNo mode, got: {}", sql);
    assert!(!sql.contains("DEFAULT false"), "raw XML value leaked through unconverted: {}", sql);
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
    assert!(sql.contains("DEFAULT 1"), "expected bit literal 1 for native mode, got: {}", sql);
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
    assert!(sql.contains("DEFAULT 'Y'"), "expected 'Y' literal for YN mode, got: {}", sql);
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
    assert!(sql.contains("SET DEFAULT 'Yes'"), "expected 'Yes' literal for YesNo mode, got: {}", sql);
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
    assert!(sql.contains("DEFAULT 'N'"), "expected 'N' literal for YN mode, got: {}", sql);
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
    assert!(sql.contains("ALTER TABLE old_users RENAME TO users"));
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
    assert!(sql.contains("RENAME COLUMN first_name TO full_name"));
    assert!(sql.contains("DROP COLUMN first_name"));
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
    assert!(sql.contains("DROP COLUMN legacy_field"));
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
        sql.contains(&format!("DROP CONSTRAINT {}", expected_name)),
        "expected DROP CONSTRAINT {} in:\n{}", expected_name, sql
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
        sql.contains(&format!("DROP INDEX IF EXISTS {}", expected_name)),
        "expected DROP INDEX IF EXISTS {} in:\n{}", expected_name, sql
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
        sql.contains(&format!("DROP INDEX IF EXISTS {}", expected_name)),
        "expected DROP INDEX IF EXISTS {} in:\n{}", expected_name, sql
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
        sql.contains(&format!("DROP CONSTRAINT {}", expected_name)),
        "expected DROP CONSTRAINT {} in:\n{}", expected_name, sql
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
    assert!(sql.contains("DROP COLUMN old_col"));
}
