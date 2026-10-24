use crate::parser::convert::{DatabaseTarget, agg_frequency_from_str};
use crate::parser::nodes::{AggregateColumnXml, TableXml};
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

pub(crate) fn parse_table(
    table_xml: &TableXml,
    schema_name: Option<&str>,
) -> Result<Table, String> {
    let columns = parse_columns(table_xml, schema_name)?;
    let keys = parse_keys(table_xml)?;
    let indexes = parse_indexes(table_xml);
    let relations = parse_relations(table_xml)?;
    let triggers = parse_triggers(table_xml)?;
    let constraints = parse_constraints(table_xml)?;
    let aggregations = parse_aggregations(table_xml)?;
    let initial_data = parse_initial_data(table_xml)?;
    let options = parse_table_options(table_xml);

    Ok(Table::new(
        schema_name,
        table_xml.name.as_str(),
        table_xml.export_data_column.as_deref(),
        lock_escalation(table_xml)?,
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
    ))
}

fn lock_escalation(table_xml: &TableXml) -> Result<LockEscalation, String> {
    use std::str::FromStr;
    match table_xml.lock_escalation.as_deref() {
        Some(s) => {
            LockEscalation::from_str(s).map_err(|e| format!("table '{}': {}", table_xml.name, e))
        }
        None => Ok(LockEscalation::Auto),
    }
}

fn parse_columns(table_xml: &TableXml, schema_name: Option<&str>) -> Result<Vec<Column>, String> {
    let mut columns = Vec::new();

    if let Some(columns_xml) = &table_xml.columns {
        for column_xml in columns_xml.column.iter() {
            let column_type = ColumnType::from_type_name(&column_xml.r#type).map_err(|e| {
                format!(
                    "table '{}', column '{}': {}",
                    table_xml.name, column_xml.name, e
                )
            })?;
            let mut builder = ColumnBuilder::new(schema_name, &column_xml.name, column_type)
                .length(column_xml.length.unwrap_or(0))
                .scale(column_xml.scale.unwrap_or(0))
                .required(column_xml.required.unwrap_or(false));
            let mut seen_checks: Vec<Option<&str>> = Vec::new();
            for check_xml in column_xml.check.iter() {
                let target = check_xml.database_type.as_deref();
                if seen_checks.contains(&target) {
                    return Err(format!(
                        "table '{}', column '{}': more than one check for {}",
                        table_xml.name,
                        column_xml.name,
                        target.map_or("every database (no databaseType)".to_string(), |t| {
                            format!("databaseType '{t}'")
                        })
                    ));
                }
                seen_checks.push(target);
                let Some(check) = check_xml.value.clone() else {
                    continue;
                };
                match check_xml.database_type.as_deref() {
                    None => builder = builder.check_constraint(Some(check)),
                    Some(name) => {
                        // A check for a database this library never generates for (e.g. h2)
                        // cannot apply to anything, so it is dropped.
                        let context =
                            format!("table '{}', column '{}'", table_xml.name, column_xml.name);
                        if let Some(database_type) =
                            crate::parser::convert::required_database_type(Some(name), &context)?
                        {
                            builder = builder.database_check(database_type, check);
                        }
                    }
                }
            }
            let column = builder
                .default_constraint(column_xml.default_value.clone())
                .generated(column_xml.generated.clone())
                .min_value(column_xml.min_value.clone())
                .max_value(column_xml.max_value.clone())
                .enum_type(column_xml.enum_type.clone())
                .element_type(column_xml.element_type.clone())
                .build();
            columns.push(column);
        }
    }

    Ok(columns)
}

fn parse_keys(table_xml: &TableXml) -> Result<Vec<Key>, String> {
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
            if unique_key_xml.where_clause.is_some() {
                if unique_key_xml.cluster == Some(true) {
                    return Err(format!(
                        "table '{}': a filtered/partial unique index cannot be clustered",
                        table_xml.name
                    ));
                }
                continue;
            }
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

    Ok(keys)
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
            if let Some(s) = &index_xml.where_clause {
                key_builder = key_builder.filter(s);
            }
            indexes.push(key_builder.build());
        }

        for unique_key_xml in keys_xml.uniques.iter().filter(|u| u.where_clause.is_some()) {
            let mut key_builder = KeyBuilder::new(KeyType::Index);
            for kc in unique_key_xml.columns.iter() {
                key_builder = key_builder.add_column(&kc.name);
            }
            key_builder = key_builder.unique(true);
            if let Some(s) = &unique_key_xml.where_clause {
                key_builder = key_builder.filter(s);
            }
            indexes.push(key_builder.build());
        }
    }

    indexes
}

fn parse_relation_type(
    table_xml: &TableXml,
    target_table: &str,
    type_str: &str,
) -> Result<RelationType, String> {
    match type_str.to_ascii_lowercase().as_str() {
        "cascade" => Ok(RelationType::Cascade),
        "enforce" => Ok(RelationType::Enforce),
        "setnull" => Ok(RelationType::SetNull),
        "donothing" => Ok(RelationType::DoNothing),
        other => Err(format!(
            "table '{}': relation to '{}' has an unrecognized type '{}' (expected cascade, enforce, setnull, or donothing)",
            table_xml.name, target_table, other
        )),
    }
}

fn parse_relations(table_xml: &TableXml) -> Result<Vec<Relation>, String> {
    let mut relations = Vec::new();

    if let Some(relations_xml) = &table_xml.relations {
        for relation_xml in relations_xml.relation.iter() {
            let relation_type =
                parse_relation_type(table_xml, &relation_xml.table, &relation_xml.r#type)?;
            relations.push(Relation::new(
                relation_xml.table.clone(),
                relation_xml.column.clone(),
                table_xml.name.clone(),
                relation_xml.src.clone(),
                relation_type,
                relation_xml.disable_usage_checking.unwrap_or(false),
            ));
        }

        for composite_relation_xml in relations_xml.composite_relation.iter() {
            let relation_type = parse_relation_type(
                table_xml,
                &composite_relation_xml.table,
                &composite_relation_xml.r#type,
            )?;
            let column_pairs: Vec<(String, String)> = composite_relation_xml
                .column
                .iter()
                .map(|c| (c.src.clone(), c.name.clone()))
                .collect();
            relations.push(Relation::new_composite(
                composite_relation_xml.table.clone(),
                table_xml.name.clone(),
                column_pairs,
                relation_type,
                composite_relation_xml
                    .disable_usage_checking
                    .unwrap_or(false),
            )?);
        }
    }

    Ok(relations)
}

fn parse_triggers(table_xml: &TableXml) -> Result<Vec<Trigger>, String> {
    let mut triggers = Vec::new();

    if let Some(triggers_xml) = &table_xml.triggers {
        for trigger_xml in triggers_xml.update.iter() {
            let Some(database_type) = crate::parser::convert::required_database_type(
                Some(&trigger_xml.database_type),
                &format!("update trigger on table '{}'", table_xml.name),
            )?
            else {
                continue;
            };
            triggers.push(Trigger::new(
                &trigger_xml.sql,
                TriggerType::Update,
                database_type,
            ));
        }

        for trigger_xml in triggers_xml.delete.iter() {
            let Some(database_type) = crate::parser::convert::required_database_type(
                Some(&trigger_xml.database_type),
                &format!("delete trigger on table '{}'", table_xml.name),
            )?
            else {
                continue;
            };
            triggers.push(Trigger::new(
                &trigger_xml.sql,
                TriggerType::Delete,
                database_type,
            ));
        }
    }

    Ok(triggers)
}

fn parse_constraints(table_xml: &TableXml) -> Result<Vec<Constraint>, String> {
    let mut constraints = Vec::new();

    if let Some(constraints_xml) = &table_xml.constraints {
        for constraint_xml in constraints_xml.constraint.iter() {
            let target = crate::parser::convert::optional_database_type(
                constraint_xml.database_type.as_deref(),
                &format!(
                    "constraint '{}' on table '{}'",
                    constraint_xml.name, table_xml.name
                ),
            )?;
            constraints.push(match target {
                DatabaseTarget::Every => {
                    Constraint::for_every_database(&constraint_xml.name, &constraint_xml.sql)
                }
                DatabaseTarget::Only(database_type) => {
                    Constraint::new(&constraint_xml.name, &constraint_xml.sql, database_type)
                }
                DatabaseTarget::Ignored => continue,
            });
        }
    }

    Ok(constraints)
}

fn parse_aggregations(table_xml: &TableXml) -> Result<Vec<Aggregation>, String> {
    let mut aggregations = Vec::new();

    if let Some(aggregations_xml) = &table_xml.aggregations {
        for aggregate_xml in aggregations_xml.aggregate.iter() {
            let cols: Vec<AggregationColumn> = aggregate_xml
                .columns
                .iter()
                .map(|column_xml| match column_xml {
                    AggregateColumnXml::Sum(sum_xml) => AggregationColumn::new(
                        AggregationType::Sum,
                        &sum_xml.source_column,
                        &sum_xml.destination_column,
                    ),
                    AggregateColumnXml::Count(count_xml) => AggregationColumn::new(
                        AggregationType::Count,
                        "",
                        &count_xml.destination_column,
                    ),
                })
                .collect();
            let mut groups: Vec<AggregationGroup> = Vec::new();
            for group_column_xml in aggregate_xml.group.column.iter() {
                groups.push(AggregationGroup::new(
                    group_column_xml.source.clone(),
                    group_column_xml.destination.clone(),
                    group_column_xml.source_derived_from.clone(),
                ));
            }
            let freq = agg_frequency_from_str(&aggregate_xml.frequency).map_err(|e| {
                format!(
                    "table '{}', aggregation to '{}': {}",
                    table_xml.name, aggregate_xml.destination_table, e
                )
            })?;
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

    Ok(aggregations)
}

fn parse_initial_data(table_xml: &TableXml) -> Result<Vec<InitialData>, String> {
    let mut initial_data = Vec::new();

    if let Some(intial_data_xml) = &table_xml.initial_data {
        for initial_sql_xml in intial_data_xml.sql.iter() {
            let database_type = match crate::parser::convert::optional_database_type(
                initial_sql_xml.database_type.as_deref(),
                &format!("initialData on table '{}'", table_xml.name),
            )? {
                DatabaseTarget::Every => None,
                DatabaseTarget::Only(database_type) => Some(database_type),
                DatabaseTarget::Ignored => continue,
            };
            initial_data.push(InitialData::new(&initial_sql_xml.sql, database_type));
        }
    }

    Ok(initial_data)
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
