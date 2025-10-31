use crate::parser::convert::agg_frequency_from_str;
use crate::parser::nodes::TableXml;
use schema_model::builder::{ColumnBuilder, KeyBuilder};
use schema_model::model::aggregation::{
    Aggregation, AggregationColumn, AggregationGroup, AggregationType,
};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::constraint::Constraint;
use schema_model::model::initial_data::InitialData;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::table::Table;
use schema_model::model::trigger::Trigger;
use schema_model::model::types::{KeyType, LockEscalation, RelationType, TableOption, TriggerType};

pub(crate) fn parse_table(table_xml: &TableXml, schema_name: Option<&str>) -> Table {
    let columns = parse_columns(table_xml, schema_name);
    let keys = parse_keys(table_xml);
    let indexes = parse_indexes(table_xml);
    let relations = parse_relations(table_xml);
    let triggers = parse_triggers(table_xml);
    let constraints = parse_constraints(table_xml);
    let aggregations = parse_aggregations(table_xml);
    let initial_data = parse_initial_data(table_xml);
    let options = parse_table_options(table_xml);

    Table::new(
        schema_name,
        table_xml.name.as_str(),
        table_xml.export_data_column.as_deref(),
        lock_escalation(table_xml),
        table_xml.no_export.unwrap_or(false),
        columns,
        keys,
        indexes,
        relations,
        triggers,
        constraints,
        initial_data,
        options,
        aggregations,
    )
}

fn lock_escalation(table_xml: &TableXml) -> LockEscalation {
    match table_xml
        .lock_escalation
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
    {
        Some(s) if s == "auto" => LockEscalation::Auto,
        _ => LockEscalation::Auto,
    }
}


fn parse_columns(table_xml: &TableXml, schema_name: Option<&str>) -> Vec<Column> {
    let mut columns = Vec::new();

    if let Some(columns_xml) = &table_xml.columns {
        for column_xml in columns_xml.column.iter() {
            let column_type = ColumnType::from_type_name(&column_xml.r#type)
                .unwrap_or_else(|e| panic!("column '{}' type error: {}", column_xml.name, e));
            let column = ColumnBuilder::new(schema_name, &column_xml.name, column_type)
                .length(column_xml.length.unwrap_or(0))
                .scale(column_xml.scale.unwrap_or(0))
                .required(column_xml.required.unwrap_or(false))
                .build();
            columns.push(column);
        }
    }

    columns
}

fn parse_keys(table_xml: &TableXml) -> Vec<Key> {
    let mut keys = Vec::new();

    if let Some(keys_xml) = &table_xml.keys {
        if let Some(primary_key_xml) = &keys_xml.primary {
            let mut kb = KeyBuilder::new(KeyType::Primary);
            for kc in primary_key_xml.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            if let Some(c) = primary_key_xml.cluster {
                kb = kb.cluster(c);
            }
            keys.push(kb.build());
        }
        for unique_key_xml in keys_xml.uniques.iter() {
            let mut kb = KeyBuilder::new(KeyType::Unique);
            for kc in unique_key_xml.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            if let Some(c) = unique_key_xml.cluster {
                kb = kb.cluster(c);
            }
            keys.push(kb.build());
        }
    }

    keys
}

fn parse_indexes(table_xml: &TableXml) -> Vec<Key> {
    let mut indexes = Vec::new();

    if let Some(keys_xml) = &table_xml.keys {
        for index_xml in keys_xml.indexes.iter() {
            let mut key_builder = KeyBuilder::new(KeyType::Index);
            for key_column_xml in index_xml.columns.iter() {
                key_builder = key_builder.add_column(&key_column_xml.name);
            }
            if let Some(s) = &index_xml.include {
                key_builder = key_builder.include(s);
            }
            if let Some(v) = index_xml.compress {
                key_builder = key_builder.compress(v);
            }
            if let Some(v) = index_xml.unique {
                key_builder = key_builder.unique(v);
            }
            indexes.push(key_builder.build());
        }
    }

    indexes
}

fn parse_relations(table_xml: &TableXml) -> Vec<Relation> {
    let mut relations = Vec::new();

    if let Some(relations_xml) = &table_xml.relations {
        for relation_xml in relations_xml.relation.iter() {
            let relation_type = match relation_xml.r#type.to_ascii_lowercase().as_str() {
                "cascade" => RelationType::Cascade,
                "enforce" => RelationType::Enforce,
                "setnull" => RelationType::SetNull,
                "donothing" => RelationType::DoNothing,
                _ => RelationType::Enforce,
            };
            relations.push(Relation::new(
                relation_xml.table.clone(),
                relation_xml.column.clone(),
                table_xml.name.clone(),
                relation_xml.src.clone(),
                relation_type,
                relation_xml.disable_usage_checking.unwrap_or(false),
            ));
        }
    }

    relations
}

fn parse_triggers(table_xml: &TableXml) -> Vec<Trigger> {
    let mut triggers = Vec::new();

    if let Some(triggers_xml) = &table_xml.triggers {
        for trigger_xml in triggers_xml.update.iter() {
            if let Some(database_type) =
                crate::parser::convert::str_to_database_type(Some(&trigger_xml.database_type))
            {
                triggers.push(Trigger::new(
                    &trigger_xml.sql,
                    TriggerType::Update,
                    database_type,
                ));
            }
        }

        for trigger_xml in triggers_xml.delete.iter() {
            if let Some(dt) =
                crate::parser::convert::str_to_database_type(Some(&trigger_xml.database_type))
            {
                triggers.push(Trigger::new(&trigger_xml.sql, TriggerType::Delete, dt));
            }
        }
    }

    triggers
}

fn parse_constraints(table_xml: &TableXml) -> Vec<Constraint> {
    let mut constraints = Vec::new();

    if let Some(constraints_xml) = &table_xml.constraints {
        for constraint_xml in constraints_xml.constraint.iter() {
            if let Some(dt) = crate::parser::convert::str_to_database_type(
                constraint_xml.database_type.as_deref(),
            ) {
                constraints.push(Constraint::new(
                    &constraint_xml.name,
                    &constraint_xml.sql,
                    dt,
                ));
            }
        }
    }

    constraints
}

fn parse_aggregations(table_xml: &TableXml) -> Vec<Aggregation> {
    let mut aggregations = Vec::new();

    if let Some(aggregations_xml) = &table_xml.aggregations {
        for aggregate_xml in aggregations_xml.aggregate.iter() {
            let mut cols: Vec<AggregationColumn> = Vec::new();
            for sum_xml in aggregate_xml.sum.iter() {
                cols.push(AggregationColumn::new(
                    AggregationType::Sum,
                    &sum_xml.source_column,
                    &sum_xml.destination_column,
                ));
            }
            for count_xml in aggregate_xml.count.iter() {
                cols.push(AggregationColumn::new(
                    AggregationType::Count,
                    "",
                    &count_xml.destination_column,
                ));
            }
            let mut groups: Vec<AggregationGroup> = Vec::new();
            for group_column_xml in aggregate_xml.group.column.iter() {
                groups.push(AggregationGroup::new(
                    group_column_xml.source.clone(),
                    group_column_xml.destination.clone(),
                    group_column_xml.source_derived_from.clone(),
                ));
            }
            let freq = agg_frequency_from_str(&aggregate_xml.frequency);
            aggregations.push(Aggregation::new(
                aggregate_xml.destination_table.clone(),
                aggregate_xml.date_column.clone(),
                aggregate_xml.criteria.clone(),
                aggregate_xml.timestamp_column.clone(),
                freq,
                cols,
                groups,
            ));
        }
    }

    aggregations
}

fn parse_initial_data(table_xml: &TableXml) -> Vec<InitialData> {
    let mut initial_data = Vec::new();

    if let Some(intial_data_xml) = &table_xml.initial_data {
        for initial_sql_xml in intial_data_xml.sql.iter() {
            let database_type = initial_sql_xml
                .database_type
                .as_ref()
                .and_then(|v| crate::parser::convert::str_to_database_type(Some(v.as_str())));
            initial_data.push(InitialData::new(&initial_sql_xml.sql, database_type));
        }
    }

    initial_data
}

fn parse_table_options(table_xml: &TableXml) -> Vec<TableOption> {
    let mut options = Vec::new();

    if table_xml.data_opt.unwrap_or(false) {
        options.push(TableOption::Data);
    }
    if table_xml.no_export.unwrap_or(false) {
        options.push(TableOption::NoExport);
    }
    if table_xml.compress.unwrap_or(false) {
        options.push(TableOption::Compress);
    }

    options
}
