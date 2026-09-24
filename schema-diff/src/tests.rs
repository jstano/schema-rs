use schema_model::builder::column::ColumnBuilder;
use schema_model::builder::key::KeyBuilder;
use schema_model::builder::schema::SchemaBuilder;
use schema_model::builder::table::TableBuilder;
use schema_model::model::column_type::ColumnType;
use schema_model::model::constraint::Constraint;
use schema_model::model::enum_type::{EnumType, EnumValue};
use schema_model::model::function::Function;
use schema_model::model::initial_data::InitialData;
use schema_model::model::other_sql::OtherSql;
use schema_model::model::procedure::Procedure;
use schema_model::model::relation::Relation;
use schema_model::model::trigger::Trigger;
use schema_model::model::types::{DatabaseType, KeyType, OtherSqlOrder, RelationType, TriggerType};
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
fn no_composite_relation_change_when_column_pairs_match() {
    let make = || {
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "assignment")
                    .add_column(ColumnBuilder::new(Some("s"), "parent_id", ColumnType::Int).build())
                    .add_column(ColumnBuilder::new(Some("s"), "property_id", ColumnType::Int).build())
                    .add_relation(
                        Relation::new_composite(
                            "assignment",
                            "assignment",
                            vec![("parent_id", "id"), ("property_id", "property_id")],
                            RelationType::Cascade,
                            false,
                        )
                        .unwrap(),
                    )
                    .build(),
            )
            .build()
    };

    let cs = SchemaDiffEngine::diff(&make(), &make());
    assert!(cs.is_empty());
}

#[test]
fn detects_composite_relation_column_order_change() {
    let make = |pairs: Vec<(&str, &str)>| {
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "assignment")
                    .add_column(ColumnBuilder::new(Some("s"), "parent_id", ColumnType::Int).build())
                    .add_column(ColumnBuilder::new(Some("s"), "property_id", ColumnType::Int).build())
                    .add_relation(
                        Relation::new_composite(
                            "assignment",
                            "assignment",
                            pairs,
                            RelationType::Cascade,
                            false,
                        )
                        .unwrap(),
                    )
                    .build(),
            )
            .build()
    };
    let old = make(vec![("parent_id", "id"), ("property_id", "property_id")]);
    let new = make(vec![("property_id", "property_id"), ("parent_id", "id")]);

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropRelation { .. })));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddRelation { .. })));
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
fn detects_key_filter_change() {
    let make = |filter: Option<&str>| {
        let mut builder = KeyBuilder::new(KeyType::Index)
            .add_column("email")
            .unique(true);
        if let Some(f) = filter {
            builder = builder.filter(f);
        }
        SchemaBuilder::new(Some("s"))
            .add_table(
                TableBuilder::new(Some("s"), "users")
                    .add_column(ColumnBuilder::new(Some("s"), "email", ColumnType::Varchar).build())
                    .add_index(builder.build())
                    .build(),
            )
            .build()
    };
    let old = make(None);
    let new = make(Some("deleted_at is null"));

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

#[test]
fn detects_added_enum_value() {
    // The original repro: adding PAY_PERIOD to an enum's value list must produce a
    // ModifyEnumType, not an empty change set.
    let old = SchemaBuilder::new(Some("s"))
        .add_enum_type(EnumType::new(
            "date_range_type",
            vec![EnumValue::new("DAILY", None::<String>), EnumValue::new("WEEKLY", None::<String>)],
        ))
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_enum_type(EnumType::new(
            "date_range_type",
            vec![
                EnumValue::new("DAILY", None::<String>),
                EnumValue::new("WEEKLY", None::<String>),
                EnumValue::new("PAY_PERIOD", None::<String>),
            ],
        ))
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert_eq!(cs.len(), 1);
    match &cs.changes()[0] {
        SchemaChange::ModifyEnumType { old_enum_type, new_enum_type } => {
            assert_eq!(old_enum_type.values().len(), 2);
            assert_eq!(new_enum_type.values().len(), 3);
        }
        other => panic!("expected ModifyEnumType, got {:?}", other),
    }
}

#[test]
fn detects_added_and_dropped_enum_type() {
    let old = SchemaBuilder::new(Some("s"))
        .add_enum_type(EnumType::new("old_type", vec![EnumValue::new("A", None::<String>)]))
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_enum_type(EnumType::new("new_type", vec![EnumValue::new("B", None::<String>)]))
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddEnumType { enum_type } if enum_type.name() == "new_type")));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropEnumType { enum_type_name } if enum_type_name == "old_type")));
}

#[test]
fn detects_added_and_changed_function() {
    let old = SchemaBuilder::new(Some("s"))
        .add_functions(vec![Function::new(Some("s"), "f1", DatabaseType::Postgresql, "create function f1() returns int as $$ select 1 $$")])
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_functions(vec![Function::new(Some("s"), "f1", DatabaseType::Postgresql, "create function f1() returns int as $$ select 2 $$")])
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropFunction { function_name, .. } if function_name == "f1")));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddFunction { function } if function.sql().contains("select 2"))));
}

#[test]
fn detects_added_procedure() {
    let old = SchemaBuilder::new(Some("s")).build();
    let new = SchemaBuilder::new(Some("s"))
        .add_procedures(vec![Procedure::new(Some("s"), "p1", DatabaseType::Postgresql, "create procedure p1() as $$ begin end $$")])
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddProcedure { procedure } if procedure.name() == "p1")));
}

#[test]
fn detects_added_and_dropped_other_sql() {
    let old = SchemaBuilder::new(Some("s"))
        .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Top, "create extension if not exists citext"))
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_other_sql(OtherSql::new(DatabaseType::Postgresql, OtherSqlOrder::Bottom, "analyze"))
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::DropOtherSql { other_sql } if other_sql.sql() == "create extension if not exists citext")));
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddOtherSql { other_sql } if other_sql.sql() == "analyze")));
}

#[test]
fn detects_added_trigger_on_a_table_present_in_both() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(TableBuilder::new(Some("s"), "orders").build())
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "orders")
                .add_trigger(Trigger::new("raise notice 'x'", TriggerType::Update, DatabaseType::Postgresql))
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(c, SchemaChange::AddTrigger { table_name, .. } if table_name == "orders")));
}

#[test]
fn detects_added_initial_data_on_a_table_present_in_both() {
    let old = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "roles")
                .add_initial_data(InitialData::new("insert into roles values (1, 'admin')", None))
                .build(),
        )
        .build();
    let new = SchemaBuilder::new(Some("s"))
        .add_table(
            TableBuilder::new(Some("s"), "roles")
                .add_initial_data(InitialData::new("insert into roles values (1, 'admin')", None))
                .add_initial_data(InitialData::new("insert into roles values (2, 'user')", None))
                .build(),
        )
        .build();

    let cs = SchemaDiffEngine::diff(&old, &new);
    assert!(cs.changes().iter().any(|c| matches!(
        c,
        SchemaChange::AddInitialData { table_name, initial_data } if table_name == "roles" && initial_data.sql().contains("'user'")
    )));
    assert!(!cs.changes().iter().any(|c| matches!(c, SchemaChange::DropInitialData { .. })));
}
