use schema_model::builder::column::ColumnBuilder;
use schema_model::builder::key::KeyBuilder;
use schema_model::builder::schema::SchemaBuilder;
use schema_model::builder::table::TableBuilder;
use schema_model::model::column_type::ColumnType;
use schema_model::model::constraint::Constraint;
use schema_model::model::relation::Relation;
use schema_model::model::types::{DatabaseType, KeyType, RelationType};
use schema_model::model::view::View;

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

#[test]
fn detects_relation_type_change() {
    let make = |rt: RelationType| {
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "orders")
                    .add_column(ColumnBuilder::new(Some("s"), "customer_id", ColumnType::Int).build())
                    .add_relation(Relation::new(
                        "customers",
                        "id",
                        "orders",
                        "customer_id",
                        rt,
                        false,
                    ))
                    .build(),
            )
            .build()
    };
    let old = make(RelationType::Cascade);
    let new = make(RelationType::SetNull);

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropRelation { relation, .. } if relation.relation_type() == RelationType::Cascade)));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddRelation { relation, .. } if relation.relation_type() == RelationType::SetNull)));
}

#[test]
fn no_relation_change_when_type_matches() {
    let make = || {
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "orders")
                    .add_column(ColumnBuilder::new(Some("s"), "customer_id", ColumnType::Int).build())
                    .add_relation(Relation::new(
                        "customers",
                        "id",
                        "orders",
                        "customer_id",
                        RelationType::Cascade,
                        false,
                    ))
                    .build(),
            )
            .build()
    };

    let cs = SchemaDiffEngine::diff(&make(), &make());
    assert!(cs.is_empty());
}

#[test]
fn detects_key_uniqueness_change() {
    let make = |unique: bool| {
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "users")
                    .add_column(ColumnBuilder::new(Some("s"), "email", ColumnType::Varchar).build())
                    .add_index(
                        KeyBuilder::new(KeyType::Index)
                            .add_column("email")
                            .unique(unique)
                            .build(),
                    )
                    .build(),
            )
            .build()
    };
    let old = make(false);
    let new = make(true);

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropKey { .. })));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddKey { .. })));
}

#[test]
fn detects_check_constraint_body_change() {
    let make = |sql: &str| {
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "orders")
                    .add_column(ColumnBuilder::new(Some("s"), "qty", ColumnType::Int).build())
                    .add_constraint(Constraint::new("chk_qty", sql, DatabaseType::Postgresql))
                    .build(),
            )
            .build()
    };
    let old = make("qty > 0");
    let new = make("qty > 1");

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropConstraint { constraint_name, .. } if constraint_name == "chk_qty")));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddConstraint { constraint, .. } if constraint.sql() == "qty > 1")));
}

#[test]
fn detects_view_select_change() {
    let make = |sql: &str| {
        SchemaBuilder::new(Some("s"))
            .add_view(View::new(Some("s"), "v_orders", sql, None))
            .build()
    };
    let old = make("select id from orders");
    let new = make("select id, qty from orders");

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropView { view_name } if view_name == "v_orders")));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddView { view } if view.sql() == "select id, qty from orders")));
}

#[test]
fn detects_column_enum_type_change() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(
                    ColumnBuilder::new(Some("s"), "status", ColumnType::Enum)
                        .enum_type(Some("status_a".to_string()))
                        .build(),
                )
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "users")
                .add_column(
                    ColumnBuilder::new(Some("s"), "status", ColumnType::Enum)
                        .enum_type(Some("status_b".to_string()))
                        .build(),
                )
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::ModifyColumn { table_name, .. } if table_name == "users")));
}
