use schema_model::builder::column::ColumnBuilder;
use schema_model::builder::schema::SchemaBuilder;
use schema_model::builder::table::TableBuilder;
use schema_model::model::column_type::ColumnType;

use crate::change::SchemaChange;
use crate::diff_engine::SchemaDiffEngine;

#[test]
fn detects_added_table() {
    let old = SchemaBuilder::new(Some("s")).build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert_eq!(cs.len(), 1);
    assert!(matches!(&cs.changes()[0], SchemaChange::AddTable { table_name } if table_name == "users"));
}

#[test]
fn detects_dropped_table() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "orders")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s")).build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert_eq!(cs.len(), 1);
    assert!(matches!(&cs.changes()[0], SchemaChange::DropTable { table_name } if table_name == "orders"));
}

#[test]
fn detects_added_column() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "name", ColumnType::Varchar).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddColumn { column, .. } if column.name() == "name")));
}

#[test]
fn detects_dropped_column() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "name", ColumnType::Varchar).build())
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropColumn { column_name, .. } if column_name == "name")));
}

#[test]
fn detects_rename_candidates_for_dropped_column() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "first_name", ColumnType::Varchar).build())
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "full_name", ColumnType::Varchar).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    let drop = cs.changes().iter().find(|c| matches!(c, SchemaChange::DropColumn { column_name, .. } if column_name == "first_name"));
    assert!(drop.is_some());
    if let SchemaChange::DropColumn { rename_candidates, .. } = drop.unwrap() {
        assert!(rename_candidates.contains(&"full_name".to_string()));
    }
}

#[test]
fn no_rename_candidates_when_types_differ() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "age", ColumnType::Int).build())
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .add_column(ColumnBuilder::new(Some("s"), "bio", ColumnType::Varchar).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    let drop = cs.changes().iter().find(|c| matches!(c, SchemaChange::DropColumn { column_name, .. } if column_name == "age"));
    assert!(drop.is_some());
    if let SchemaChange::DropColumn { rename_candidates, .. } = drop.unwrap() {
        assert!(rename_candidates.is_empty());
    }
}

#[test]
fn detects_modified_column() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "age", ColumnType::Int).build())
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "age", ColumnType::Long).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::ModifyColumn { table_name, .. } if table_name == "users")));
}

#[test]
fn no_changes_when_identical() {
    let s1 = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .build(),
        )
        .build();
    let s2 = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(ColumnBuilder::new(Some("s"), "id", ColumnType::Int).required(true).build())
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&s1, &s2);
    assert!(cs.is_empty());
}
