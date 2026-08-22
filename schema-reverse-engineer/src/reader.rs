use crate::error::SchemaReverseEngineerError;
use crate::postgres::{columns, constraints, enums, keys, relations, tables, views};
use schema_model::builder::{ColumnBuilder, KeyBuilder, SchemaBuilder, TableBuilder};
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, ForeignKeyMode, KeyType};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

/// Introspects the given Postgres schema (`db_schema`, e.g. `"public"`) and builds a
/// `DatabaseModel` describing its tables, columns, keys, foreign keys, check/exclusion
/// constraints, enum types, and views.
pub async fn read_schema(pool: &PgPool, db_schema: &str) -> Result<DatabaseModel, SchemaReverseEngineerError> {
    let table_names = tables::list_tables(pool, db_schema).await?;
    let enum_types = enums::list_enum_types(pool, db_schema).await?;
    let enum_type_names: HashSet<String> = enum_types.iter().map(|e| e.name().to_string()).collect();

    let all_columns = columns::list_columns(pool, db_schema, &enum_type_names).await?;
    let exclusion_constraint_names = constraints::list_exclusion_constraint_names(pool, db_schema).await?;
    let mut table_keys = keys::list_keys(pool, db_schema, &exclusion_constraint_names).await?;
    let mut table_relations = relations::list_foreign_keys(pool, db_schema).await?;
    let mut table_constraints = constraints::list_constraints(pool, db_schema).await?;
    let table_views = views::list_views(pool, db_schema).await?;

    let mut columns_by_table: HashMap<String, Vec<columns::ColumnInfo>> = HashMap::new();
    for column in all_columns {
        columns_by_table.entry(column.table_name.clone()).or_default().push(column);
    }

    let mut schema_builder = SchemaBuilder::new(None::<&str>);
    for enum_type in enum_types {
        schema_builder = schema_builder.add_enum_type(enum_type);
    }
    for view in table_views {
        schema_builder = schema_builder.add_view(view);
    }

    for table_name in &table_names {
        let mut table_builder = TableBuilder::new(None::<&str>, table_name.as_str());

        for column in columns_by_table.remove(table_name).unwrap_or_default() {
            table_builder = table_builder.add_column(
                ColumnBuilder::new(None::<&str>, column.column_name.as_str(), column.column_type)
                    .length(column.length)
                    .scale(column.scale)
                    .required(column.required)
                    .default_constraint(column.default_constraint)
                    .generated(column.generated)
                    .enum_type(column.enum_type)
                    .element_type(column.element_type)
                    .build(),
            );
        }

        if let Some(keys_for_table) = table_keys.remove(table_name) {
            if let Some(primary) = keys_for_table.primary {
                table_builder = table_builder.add_key(build_key(KeyType::Primary, primary.columns));
            }
            for unique in keys_for_table.unique {
                table_builder = table_builder.add_key(build_key(KeyType::Unique, unique.columns));
            }
            for index in keys_for_table.index {
                table_builder = table_builder.add_index(build_key(KeyType::Index, index.columns));
            }
        }

        for relation in table_relations.remove(table_name).unwrap_or_default() {
            table_builder = table_builder.add_relation(relation);
        }

        for constraint in table_constraints.remove(table_name).unwrap_or_default() {
            table_builder = table_builder.add_constraint(constraint);
        }

        schema_builder = schema_builder.add_table(table_builder.build());
    }

    let schema = schema_builder.build();

    Ok(DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]))
}

fn build_key(key_type: KeyType, columns: Vec<String>) -> schema_model::model::key::Key {
    let mut builder = KeyBuilder::new(key_type);
    for column in columns {
        builder = builder.add_column(column);
    }
    builder.build()
}
