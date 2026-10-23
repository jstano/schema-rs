use crate::error::SchemaReverseEngineerError;
use crate::info::{ColumnInfo, KeyInfo, TableKeys};
use crate::postgres::{columns, constraints, enums, keys, relations, routines, tables, views};
use schema_model::builder::{ColumnBuilder, KeyBuilder, SchemaBuilder, TableBuilder};
use schema_model::model::constraint::Constraint;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::enum_type::EnumType;
use schema_model::model::function::Function;
use schema_model::model::other_sql::OtherSql;
use schema_model::model::procedure::Procedure;
use schema_model::model::relation::Relation;
use schema_model::model::types::{BooleanMode, ForeignKeyMode, KeyType};
use schema_model::model::view::View;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

/// Introspects the given Postgres schema (`db_schema`, e.g. `"public"`) and builds a
/// `DatabaseModel` describing its tables, columns, keys, foreign keys, check/exclusion
/// constraints, enum types, views, functions, and procedures.
pub async fn read_schema(
    pool: &PgPool,
    db_schema: &str,
) -> Result<DatabaseModel, SchemaReverseEngineerError> {
    let table_names = tables::list_tables(pool, db_schema).await?;
    let enum_types = enums::list_enum_types(pool, db_schema).await?;
    let enum_type_names: HashSet<String> =
        enum_types.iter().map(|e| e.name().to_string()).collect();

    let all_columns = columns::list_columns(pool, db_schema, &enum_type_names).await?;
    let exclusion_constraint_names =
        constraints::list_exclusion_constraint_names(pool, db_schema).await?;
    let table_keys = keys::list_keys(pool, db_schema, &exclusion_constraint_names).await?;
    let table_relations = relations::list_foreign_keys(pool, db_schema).await?;
    let table_constraints = constraints::list_constraints(pool, db_schema).await?;
    let table_views = views::list_views(pool, db_schema).await?;
    let (functions, procedures) = routines::list_routines(pool, db_schema).await?;

    Ok(assemble_schema(
        db_schema,
        SchemaParts {
            table_names,
            enum_types,
            columns: all_columns,
            keys: table_keys,
            relations: table_relations,
            constraints: table_constraints,
            views: table_views,
            functions,
            procedures,
            other_sql: Vec::new(),
        },
    ))
}

/// Everything a dialect reader collected for one database schema.
pub(crate) struct SchemaParts {
    pub table_names: Vec<String>,
    pub enum_types: Vec<EnumType>,
    pub columns: Vec<ColumnInfo>,
    pub keys: HashMap<String, TableKeys>,
    pub relations: HashMap<String, Vec<Relation>>,
    pub constraints: HashMap<String, Vec<Constraint>>,
    pub views: Vec<View>,
    pub functions: Vec<Function>,
    pub procedures: Vec<Procedure>,
    pub other_sql: Vec<OtherSql>,
}

/// Builds the dialect-neutral `DatabaseModel` from a reader's collected parts.
pub(crate) fn assemble_schema(db_schema: &str, parts: SchemaParts) -> DatabaseModel {
    let SchemaParts {
        table_names,
        enum_types,
        columns: all_columns,
        keys: mut table_keys,
        relations: mut table_relations,
        constraints: mut table_constraints,
        views: table_views,
        functions,
        procedures,
        other_sql,
    } = parts;

    let mut columns_by_table: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
    for column in all_columns {
        columns_by_table
            .entry(column.table_name.clone())
            .or_default()
            .push(column);
    }

    let mut schema_builder = SchemaBuilder::new(Some(db_schema));
    for enum_type in enum_types {
        schema_builder = schema_builder.add_enum_type(enum_type);
    }
    for view in table_views {
        schema_builder = schema_builder.add_view(view);
    }
    schema_builder = schema_builder
        .add_functions(functions)
        .add_procedures(procedures);
    for other in other_sql {
        schema_builder = schema_builder.add_other_sql(other);
    }

    for table_name in &table_names {
        let mut table_builder = TableBuilder::new(Some(db_schema), table_name.as_str());

        let table_columns = columns_by_table.remove(table_name).unwrap_or_default();
        for column in table_columns.iter().cloned() {
            table_builder = table_builder.add_column(
                ColumnBuilder::new(
                    Some(db_schema),
                    column.column_name.as_str(),
                    column.column_type,
                )
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

        if let Some(mut keys_for_table) = table_keys.remove(table_name) {
            promote_lone_unique_key(&mut keys_for_table, &table_columns);
            if let Some(primary) = keys_for_table.primary {
                table_builder = table_builder.add_key(build_key(KeyType::Primary, primary));
            }
            for unique in keys_for_table.unique {
                table_builder = table_builder.add_key(build_key(KeyType::Unique, unique));
            }
            for index in keys_for_table.index {
                table_builder = table_builder.add_index(build_key(KeyType::Index, index));
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

    DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
}

/// A table with no primary key but exactly one unique key gets that key promoted to primary
/// (mirrors a heuristic in the Java original). Only done when every key column is already
/// `NOT NULL`: promoting a nullable column would invent a `NOT NULL` the database doesn't have.
fn promote_lone_unique_key(keys: &mut TableKeys, table_columns: &[columns::ColumnInfo]) {
    if keys.primary.is_some() || keys.unique.len() != 1 {
        return;
    }
    let all_required = keys.unique[0].columns.iter().all(|key_column| {
        table_columns
            .iter()
            .any(|c| c.column_name.eq_ignore_ascii_case(key_column) && c.required)
    });
    if all_required {
        keys.primary = Some(keys.unique.remove(0));
    }
}

fn build_key(key_type: KeyType, info: KeyInfo) -> schema_model::model::key::Key {
    let mut builder = KeyBuilder::new(key_type);
    for column in info.columns {
        builder = builder.add_column(column);
    }
    if key_type == KeyType::Index && info.unique {
        builder = builder.unique(true);
    }
    if !info.include.is_empty() {
        builder = builder.include(info.include.join(","));
    }
    if let Some(filter) = info.filter {
        builder = builder.filter(filter);
    }
    builder.build()
}
