use schema_diff::SchemaDiffEngine;
use schema_diff::change::SchemaChange;
use schema_diff::change_set::ChangeSet;
use schema_model::builder::column::ColumnBuilder;
use schema_model::builder::{SchemaBuilder, TableBuilder};
use schema_model::model::column_type::ColumnType;
use schema_model::model::key::{Key, KeyColumn};
use schema_model::model::relation::Relation;
use schema_model::model::types::{DatabaseType, KeyType, RelationType};
use schema_model::naming::{foreign_key_name, index_name, primary_key_name, unique_key_name};

use crate::create_generator;

#[test]
fn postgresql_add_table() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::AddTable { table_name: "users".to_string() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("CREATE TABLE users"));
}

#[test]
fn postgresql_drop_table() {
    let mut cs = ChangeSet::new();
    cs.add_change(SchemaChange::DropTable { table_name: "orders".to_string() });

    let generator = create_generator(DatabaseType::Postgresql);
    let mut output = Vec::new();
    generator.generate(&cs, &mut output).unwrap();
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
    generator.generate(&cs, &mut output).unwrap();
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
    generator.generate(&cs, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("GO"));
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
    generator.generate(&cs, &mut output).unwrap();
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
    generator.generate(&cs, &mut output).unwrap();
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
    generator.generate(&cs, &mut output).unwrap();
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
    generator.generate(&change_set, &mut output).unwrap();
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
    generator.generate(&change_set, &mut output).unwrap();
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
    generator.generate(&change_set, &mut output).unwrap();
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
    generator.generate(&change_set, &mut output).unwrap();
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
    generator.generate(&cs, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    assert!(sql.contains("-- TODO: possible rename?"));
    assert!(sql.contains("sp_rename 'orders.old_col', 'new_col', 'COLUMN'"));
    assert!(sql.contains("DROP COLUMN old_col"));
}
