use schema_model::builder::{ColumnBuilder, KeyBuilder, TableBuilder};
use schema_model::model::aggregation::{Aggregation, AggregationColumn, AggregationGroup, AggregationType};
use schema_model::model::column_type::ColumnType;
use schema_model::model::constraint::Constraint;
use schema_model::model::initial_data::InitialData;
use schema_model::model::relation::Relation;
use schema_model::model::table::Table;
use schema_model::model::trigger::Trigger;
use schema_model::model::types::{KeyType, LockEscalation, RelationType, TableOption, TriggerType};
use crate::parser::convert::agg_frequency_from_str;
use crate::parser::nodes::TableXml;

pub(crate) fn parse_table(table_xml: &TableXml, schema_name: Option<&str>) -> Table {
    let lock_escalation = match table_xml.lock_escalation.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "auto" => LockEscalation::Auto,
        _ => LockEscalation::Auto,
    };

    let mut table_builder = TableBuilder::new(schema_name, &table_xml.name).no_export(table_xml.no_export.unwrap_or(false));
    if let Some(c) = &table_xml.export_data_column {
        table_builder = table_builder.export_date_column(c.clone());
    }
    table_builder = table_builder.lock_escalation(lock_escalation);

    if let Some(cols) = &table_xml.columns {
        for column_xml in cols.column.iter() {
            let ctype = ColumnType::from_type_name(&column_xml.r#type)
                .unwrap_or_else(|e| panic!("column '{}' type error: {}", column_xml.name, e));
            let col = ColumnBuilder::new(schema_name, &column_xml.name, ctype)
                .length(column_xml.length.unwrap_or(0))
                .scale(column_xml.scale.unwrap_or(0))
                .required(column_xml.required.unwrap_or(false))
                .build();
            table_builder = table_builder.add_column(col);
        }
    }

    if let Some(keys_xml) = &table_xml.keys {
        if let Some(primary_key_xml) = &keys_xml.primary {
            let mut kb = KeyBuilder::new(KeyType::Primary);
            for kc in primary_key_xml.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            if let Some(c) = primary_key_xml.cluster {
                kb = kb.cluster(c);
            }
            table_builder = table_builder.add_key(kb.build());
        }
        for unique_key_xml in keys_xml.uniques.iter() {
            let mut kb = KeyBuilder::new(KeyType::Unique);
            for kc in unique_key_xml.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            if let Some(c) = unique_key_xml.cluster {
                kb = kb.cluster(c);
            }
            table_builder = table_builder.add_key(kb.build());
        }
        for index_xml in keys_xml.indexes.iter() {
            let mut kb = KeyBuilder::new(KeyType::Index);
            for kc in index_xml.columns.iter() {
                kb = kb.add_column(&kc.name);
            }
            if let Some(s) = &index_xml.include { kb = kb.include(s); }
            if let Some(v) = index_xml.compress { kb = kb.compress(v); }
            if let Some(v) = index_xml.unique { kb = kb.unique(v); }
            table_builder = table_builder.add_index(kb.build());
        }
    }

    if let Some(relations_xml) = &table_xml.relations {
        for relation_xml in relations_xml.relation.iter() {
            let rtype = match relation_xml.r#type.to_ascii_lowercase().as_str() {
                "cascade" => RelationType::Cascade,
                "enforce" => RelationType::Enforce,
                "setnull" => RelationType::SetNull,
                "donothing" => RelationType::DoNothing,
                _ => RelationType::Enforce,
            };
            table_builder = table_builder.add_relation(Relation::new(
                relation_xml.table.clone(),
                relation_xml.column.clone(),
                table_xml.name.clone(),
                relation_xml.src.clone(),
                rtype,
                relation_xml.disable_usage_checking.unwrap_or(false),
            ));
        }
    }

    let mut table = table_builder.build();

    if let Some(triggers_xml) = &table_xml.triggers {
        for tr in triggers_xml.update.iter() {
            if let Some(dt) = crate::parser::convert::str_to_database_type_opt(Some(&tr.database_type)) {
                table.triggers_mut().push(Trigger::new(&tr.sql, TriggerType::Update, dt));
            }
        }
        for trigger_xml in triggers_xml.delete.iter() {
            if let Some(dt) = crate::parser::convert::str_to_database_type_opt(Some(&trigger_xml.database_type)) {
                table.triggers_mut().push(Trigger::new(&trigger_xml.sql, TriggerType::Delete, dt));
            }
        }
    }

    if let Some(constraints_xml) = &table_xml.constraints {
        for constraint_xml in constraints_xml.constraint.iter() {
            if let Some(dt) = crate::parser::convert::str_to_database_type_opt(constraint_xml.database_type.as_deref()) {
                table.constraints_mut().push(Constraint::new(&constraint_xml.name, &constraint_xml.sql, dt));
            }
        }
    }

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
            table.aggregations_mut().push(Aggregation::new(
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

    if let Some(intial_data_xml) = &table_xml.initial_data {
        for s in intial_data_xml.sql.iter() {
            let dt = s
                .database_type
                .as_ref()
                .and_then(|v| crate::parser::convert::str_to_database_type_opt(Some(v.as_str())));
            table.initial_data_mut().push(InitialData::new(&s.sql, dt));
        }
    }

    // Table options from flags
    if table_xml.data_opt.unwrap_or(false) {
        table.options_mut().push(TableOption::Data);
    }
    if table_xml.no_export.unwrap_or(false) {
        table.options_mut().push(TableOption::NoExport);
    }
    if table_xml.compress.unwrap_or(false) {
        table.options_mut().push(TableOption::Compress);
    }

    table
}
