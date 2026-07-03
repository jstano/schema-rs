use schema_diff::change::SchemaChange;
use schema_diff::change_set::ChangeSet;
use schema_model::builder::column::ColumnBuilder;
use schema_model::model::column_type::ColumnType;
use schema_model::model::types::DatabaseType;

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
