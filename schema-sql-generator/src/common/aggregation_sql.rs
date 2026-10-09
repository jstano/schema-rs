use schema_model::model::aggregation::{Aggregation, AggregationColumn, AggregationType};
use schema_model::model::table::Table;
use schema_model::model::types::DatabaseType;

/// Schema-qualifies an aggregation's destination table with the source table's schema when
/// the XML names it bare; a name that already contains a `.` is used as written.
pub fn destination_table_name(
    database_type: DatabaseType,
    table: &Table,
    aggregation: &Aggregation,
) -> String {
    let destination = aggregation.destination_table();
    if destination.contains('.') {
        destination.to_string()
    } else {
        database_type.qualified_name(table.schema_name(), destination)
    }
}

/// The value a source row contributes to an aggregation column: a count contributes `1`
/// per row, a sum contributes the (null-as-zero) source column.
pub fn row_value(column: &AggregationColumn) -> String {
    match column.aggregation_type() {
        AggregationType::Count => "1".to_string(),
        AggregationType::Sum => format!("coalesce({}, 0)", column.source_column()),
    }
}

/// The criteria as a parenthesised predicate, or `None` when absent/blank.
pub fn criteria_predicate(aggregation: &Aggregation) -> Option<String> {
    aggregation
        .criteria()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| format!("({})", c))
}

/// Source columns whose change can alter an aggregation's contribution, or `None` when that
/// can't be known (the free-form criteria may read any column), in which case an update must
/// always be re-aggregated. A group's `sourceDerivedFrom` names the column its (possibly
/// expression-valued) `source` is derived from.
pub fn watched_columns(aggregation: &Aggregation) -> Option<Vec<String>> {
    if criteria_predicate(aggregation).is_some() {
        return None;
    }

    let mut columns: Vec<String> = Vec::new();
    for column in aggregation.aggregation_columns() {
        if column.aggregation_type() == AggregationType::Sum {
            columns.push(column.source_column().to_string());
        }
    }
    for group in aggregation.aggregation_groups() {
        match group.source_derived_from().map(str::trim) {
            Some(derived) if !derived.is_empty() => columns.push(derived.to_string()),
            _ => columns.push(group.source().to_string()),
        }
    }
    columns.push(aggregation.date_column().to_string());
    Some(columns)
}
