//! Maintains `aggregation`'s destination table from statement-level trigger events on `table`.
//!
//! The destination holds one row per (group columns, date bucket of `dateColumn`). Each
//! trigger turns the `inserted`/`deleted` pseudo-tables into per-bucket deltas and applies
//! them with a `merge` (insert/update triggers) or an `update` (delete trigger). A group's
//! `source` is a column or SQL expression over the source row; its value lands in the group's
//! `destination` column. Week buckets follow the session's `DATEFIRST`.

use crate::common::aggregation_sql::{
    criteria_predicate, destination_table_name, row_value, watched_columns,
};
use schema_model::model::aggregation::{Aggregation, AggregationFrequency};
use schema_model::model::table::Table;
use schema_model::model::types::DatabaseType;

fn date_bucket(aggregation: &Aggregation) -> String {
    let d = aggregation.date_column();
    match aggregation.aggregation_frequency() {
        AggregationFrequency::Daily => format!("cast({} as date)", d),
        AggregationFrequency::Weekly => {
            format!("cast(dateadd(d, -datepart(dw, {d}) + 1, {d}) as date)")
        }
        AggregationFrequency::Monthly => {
            format!("cast(dateadd(d, -datepart(d, {d}) + 1, {d}) as date)")
        }
        AggregationFrequency::Yearly => {
            format!("cast(dateadd(d, -datepart(dy, {d}) + 1, {d}) as date)")
        }
    }
}

/// CTE `alias` projecting each row of `pseudo_table` (`inserted`/`deleted`) into the
/// destination's shape: one value per aggregation column, the group values, and the bucket.
fn source_cte(aggregation: &Aggregation, pseudo_table: &str, alias: &str) -> String {
    let mut items: Vec<String> = Vec::new();
    for column in aggregation.aggregation_columns() {
        items.push(format!(
            "{} as {}",
            row_value(column),
            column.destination_column()
        ));
    }
    for group in aggregation.aggregation_groups() {
        items.push(format!("{} as {}", group.source(), group.destination()));
    }
    items.push(format!(
        "{} as {}",
        date_bucket(aggregation),
        aggregation.date_column()
    ));

    let mut sql = format!(
        "{} as (select {} from {}",
        alias,
        items.join(", "),
        pseudo_table
    );
    if let Some(criteria) = criteria_predicate(aggregation) {
        sql.push_str(" where ");
        sql.push_str(&criteria);
    }
    sql.push(')');
    sql
}

fn group_and_date_columns(aggregation: &Aggregation) -> Vec<String> {
    let mut columns: Vec<String> = aggregation
        .aggregation_groups()
        .iter()
        .map(|group| group.destination().to_string())
        .collect();
    columns.push(aggregation.date_column().to_string());
    columns
}

fn join_on(aggregation: &Aggregation, left: &str, right: &str) -> String {
    group_and_date_columns(aggregation)
        .iter()
        .map(|c| format!("{}.{} = {}.{}", left, c, right, c))
        .collect::<Vec<_>>()
        .join(" and ")
}

/// Lines for the body of the `for insert, update` trigger.
pub fn upsert_aggregation_sql(table: &Table, aggregation: &Aggregation) -> Vec<String> {
    let destination = destination_table_name(DatabaseType::SqlServer, table, aggregation);
    let timestamp = aggregation.time_stamp_column();
    let keys = group_and_date_columns(aggregation);
    let columns: Vec<&str> = aggregation
        .aggregation_columns()
        .iter()
        .map(|c| c.destination_column())
        .collect();

    // Inserted rows count positively, deleted rows negatively.
    let signed_item = |sign: &str, column: &str| format!("{}{} as {}", sign, column, column);
    let branch = |sign: &str, alias: &str| {
        let mut items: Vec<String> = columns.iter().map(|c| signed_item(sign, c)).collect();
        items.extend(keys.iter().cloned());
        format!("select {} from {}", items.join(", "), alias)
    };
    let delta_cte = format!(
        "delta as ({} union all {})",
        branch("", "I"),
        branch("-", "D")
    );

    let mut aggregated_items: Vec<String> = columns
        .iter()
        .map(|c| format!("sum(coalesce({c}, 0)) as {c}"))
        .collect();
    aggregated_items.extend(keys.iter().cloned());
    let aggregated_cte = format!(
        "aggregated as (select {} from delta group by {})",
        aggregated_items.join(", "),
        keys.join(", ")
    );

    let mut matched_items: Vec<String> = columns
        .iter()
        .map(|c| format!("target.{c} = target.{c} + source.{c}"))
        .collect();
    matched_items.push(format!("target.{} = getdate()", timestamp));

    let mut insert_columns = vec![timestamp.to_string()];
    let mut insert_values = vec!["getdate()".to_string()];
    for key in &keys {
        insert_columns.push(key.clone());
        insert_values.push(format!("source.{}", key));
    }
    for column in &columns {
        insert_columns.push(column.to_string());
        insert_values.push(format!("source.{}", column));
    }

    let mut lines = vec!["   ;".to_string()];
    if let Some(watched) = watched_columns(aggregation) {
        let guard = watched
            .iter()
            .map(|c| format!("update({})", c))
            .collect::<Vec<_>>()
            .join(" or ");
        lines.push(format!("   if {}", guard));
    }
    lines.push(format!(
        "   with {},\n      {},\n      {},\n      {}",
        source_cte(aggregation, "deleted", "D"),
        source_cte(aggregation, "inserted", "I"),
        delta_cte,
        aggregated_cte
    ));
    lines.push(format!(
        "   merge {} as target using aggregated as source on ({})\n   when matched then update set\n      {}\n   when not matched then insert ({})\n      values ({});",
        destination,
        join_on(aggregation, "target", "source"),
        matched_items.join(",\n      "),
        insert_columns.join(", "),
        insert_values.join(", ")
    ));
    lines
}

/// Lines for the body of the `for delete` trigger.
pub fn delete_aggregation_sql(table: &Table, aggregation: &Aggregation) -> Vec<String> {
    let destination = destination_table_name(DatabaseType::SqlServer, table, aggregation);
    let timestamp = aggregation.time_stamp_column();
    let keys = group_and_date_columns(aggregation);

    let mut aggregated_items: Vec<String> = aggregation
        .aggregation_columns()
        .iter()
        .map(|c| {
            let name = c.destination_column();
            format!("sum({name}) as {name}")
        })
        .collect();
    aggregated_items.extend(keys.iter().cloned());

    let mut set_items: Vec<String> = aggregation
        .aggregation_columns()
        .iter()
        .map(|c| {
            let name = c.destination_column();
            format!("dest.{name} = dest.{name} - aggregated.{name}")
        })
        .collect();
    set_items.push(format!("dest.{} = getdate()", timestamp));

    vec![
        "   ;".to_string(),
        format!(
            "   with {},\n      aggregated as (select {} from D group by {})\n   update dest set\n      {}\n   from {} as dest join aggregated on {};",
            source_cte(aggregation, "deleted", "D"),
            aggregated_items.join(", "),
            keys.join(", "),
            set_items.join(",\n      "),
            destination,
            join_on(aggregation, "dest", "aggregated")
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::common::test_support::make_context;
    use crate::common::trigger_generator::TriggerGenerator;
    use crate::sqlserver::sqlserver_trigger_generator::SqlServerTriggerGenerator;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::aggregation::{
        Aggregation, AggregationColumn, AggregationFrequency, AggregationGroup, AggregationType,
    };
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    fn generate(criteria: Option<&str>, frequency: AggregationFrequency) -> String {
        let aggregation = Aggregation::new(
            "daily_sales",
            "sold",
            criteria,
            "updated_at",
            frequency,
            vec![
                AggregationColumn::new(AggregationType::Sum, "amount", "total"),
                AggregationColumn::new(AggregationType::Count, "", "sale_count"),
            ],
            vec![AggregationGroup::new(
                "store",
                "store_id",
                Some("store_ref"),
            )],
        );
        let table = TableBuilder::new(Some("app"), "sale")
            .add_column(ColumnBuilder::new(None::<&str>, "amount", ColumnType::Int).build())
            .add_aggregation(aggregation)
            .build();
        let schema = SchemaBuilder::new(Some("app")).add_table(table).build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::SqlServer);
        SqlServerTriggerGenerator::new(ctx).output_triggers();
        buffer.contents()
    }

    #[test]
    fn insert_update_trigger_merges_inserted_minus_deleted_deltas() {
        let sql = generate(None, AggregationFrequency::Monthly);

        assert!(
            sql.contains("cast(dateadd(d, -datepart(d, sold) + 1, sold) as date) as sold"),
            "{sql}"
        );
        assert!(sql.contains("delta as (select total as total, sale_count as sale_count, store_id, sold from I union all select -total as total, -sale_count as sale_count, store_id, sold from D)"), "{sql}");
        assert!(sql.contains("merge app.daily_sales as target using aggregated as source on (target.store_id = source.store_id and target.sold = source.sold)"), "{sql}");
        assert!(
            sql.contains("target.total = target.total + source.total"),
            "{sql}"
        );
        assert!(sql.contains("target.updated_at = getdate()"), "{sql}");
        assert!(
            sql.contains(
                "when not matched then insert (updated_at, store_id, sold, total, sale_count)"
            ),
            "{sql}"
        );
    }

    #[test]
    fn update_only_reaggregates_when_a_contributing_column_changed() {
        let sql = generate(None, AggregationFrequency::Daily);

        assert!(
            sql.contains("if update(amount) or update(store_ref) or update(sold)\n"),
            "{sql}"
        );
    }

    #[test]
    fn criteria_filters_both_pseudo_tables_and_disables_the_update_shortcut() {
        let sql = generate(Some("voided = 0"), AggregationFrequency::Weekly);

        assert!(sql.contains("from deleted where (voided = 0)"), "{sql}");
        assert!(sql.contains("from inserted where (voided = 0)"), "{sql}");
        assert!(!sql.contains("if update("), "{sql}");
    }

    #[test]
    fn delete_trigger_subtracts_the_deleted_rows() {
        let sql = generate(None, AggregationFrequency::Yearly);

        assert!(
            sql.contains("create trigger sale_delete on app.sale for delete as"),
            "{sql}"
        );
        assert!(sql.contains("aggregated as (select sum(total) as total, sum(sale_count) as sale_count, store_id, sold from D group by store_id, sold)"), "{sql}");
        assert!(
            sql.contains("dest.total = dest.total - aggregated.total"),
            "{sql}"
        );
        assert!(sql.contains("from app.daily_sales as dest join aggregated on dest.store_id = aggregated.store_id and dest.sold = aggregated.sold;"), "{sql}");
    }
}
