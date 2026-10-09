//! Maintains `aggregation`'s destination table from row-level trigger events on `table`.
//!
//! The destination holds one row per (group columns, date bucket of `dateColumn`) and must
//! have a unique key over exactly those columns, which the insert's `on conflict` targets.
//! A group's `source` is a column or SQL expression over the source row; its value lands in
//! the group's `destination` column. Weeks start on Monday (`date_trunc`).

use crate::common::aggregation_sql::{
    criteria_predicate, destination_table_name, row_value, watched_columns,
};
use schema_model::model::aggregation::{Aggregation, AggregationFrequency};
use schema_model::model::table::Table;
use schema_model::model::types::DatabaseType;

fn date_bucket(aggregation: &Aggregation) -> String {
    let date_column = aggregation.date_column();
    match aggregation.aggregation_frequency() {
        AggregationFrequency::Daily => format!("cast({} as date)", date_column),
        AggregationFrequency::Weekly => {
            format!("cast(date_trunc('week', {}) as date)", date_column)
        }
        AggregationFrequency::Monthly => {
            format!("cast(date_trunc('month', {}) as date)", date_column)
        }
        AggregationFrequency::Yearly => {
            format!("cast(date_trunc('year', {}) as date)", date_column)
        }
    }
}

/// `select` over the trigger's `new`/`old` record (`record`) yielding the destination-shaped
/// contribution of that single row, or no rows when the criteria rejects it.
fn contribution_select(aggregation: &Aggregation, record: &str) -> String {
    let mut items = vec![format!(
        "{} as {}",
        date_bucket(aggregation),
        aggregation.date_column()
    )];
    for group in aggregation.aggregation_groups() {
        items.push(format!("{} as {}", group.source(), group.destination()));
    }
    for column in aggregation.aggregation_columns() {
        items.push(format!(
            "{} as {}",
            row_value(column),
            column.destination_column()
        ));
    }

    let mut sql = format!(
        "select {} from (select {}.*) as src",
        items.join(", "),
        record
    );
    if let Some(criteria) = criteria_predicate(aggregation) {
        sql.push_str(" where ");
        sql.push_str(&criteria);
    }
    sql
}

/// Statement adding the contribution of `new` to the destination (insert or increment).
fn add_new_statement(table: &Table, aggregation: &Aggregation) -> String {
    let destination = destination_table_name(DatabaseType::Postgresql, table, aggregation);
    let timestamp = aggregation.time_stamp_column();
    let date = aggregation.date_column();

    let mut insert_columns = vec![timestamp.to_string(), date.to_string()];
    let mut select_items = vec!["now()".to_string(), format!("agg.{}", date)];
    let mut conflict_columns: Vec<String> = Vec::new();
    for group in aggregation.aggregation_groups() {
        insert_columns.push(group.destination().to_string());
        select_items.push(format!("agg.{}", group.destination()));
        conflict_columns.push(group.destination().to_string());
    }
    conflict_columns.push(date.to_string());

    let mut update_items: Vec<String> = Vec::new();
    for column in aggregation.aggregation_columns() {
        let name = column.destination_column();
        insert_columns.push(name.to_string());
        select_items.push(format!("agg.{}", name));
        update_items.push(format!(
            "{} = coalesce(dest.{}, 0) + excluded.{}",
            name, name, name
        ));
    }
    update_items.push(format!("{} = excluded.{}", timestamp, timestamp));

    // `where true` keeps the parser from reading `on conflict` as a join's `on`.
    format!(
        "   insert into {} as dest ({})\n   select {} from ({}) as agg where true\n   on conflict ({}) do update set\n      {};",
        destination,
        insert_columns.join(", "),
        select_items.join(", "),
        contribution_select(aggregation, "new"),
        conflict_columns.join(", "),
        update_items.join(",\n      ")
    )
}

/// Statement subtracting the contribution of `record` (`old`) from the destination.
fn subtract_statement(table: &Table, aggregation: &Aggregation, record: &str) -> String {
    let destination = destination_table_name(DatabaseType::Postgresql, table, aggregation);
    let timestamp = aggregation.time_stamp_column();

    let mut set_items: Vec<String> = aggregation
        .aggregation_columns()
        .iter()
        .map(|column| {
            let name = column.destination_column();
            format!("{} = coalesce(dest.{}, 0) - agg.{}", name, name, name)
        })
        .collect();
    set_items.push(format!("{} = now()", timestamp));

    let mut join_items: Vec<String> = aggregation
        .aggregation_groups()
        .iter()
        .map(|group| format!("dest.{} = agg.{}", group.destination(), group.destination()))
        .collect();
    join_items.push(format!(
        "dest.{} = agg.{}",
        aggregation.date_column(),
        aggregation.date_column()
    ));

    format!(
        "   update {} as dest set\n      {}\n   from ({}) as agg\n   where {};",
        destination,
        set_items.join(",\n      "),
        contribution_select(aggregation, record),
        join_items.join(" and ")
    )
}

/// Lines for the body of the `after insert or update` trigger function.
pub fn upsert_aggregation_sql(table: &Table, aggregation: &Aggregation) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    match watched_columns(aggregation) {
        Some(columns) => {
            // `old` is unassigned on insert, so it may only be read in the `elsif` branch.
            let changed = columns
                .iter()
                .map(|c| format!("new.{} is distinct from old.{}", c, c))
                .collect::<Vec<_>>()
                .join("\n         or ");
            let add = add_new_statement(table, aggregation);
            lines.push("   if tg_op = 'INSERT' then".to_string());
            lines.push(indent(&add, 3));
            lines.push(format!("   elsif {} then", changed));
            lines.push(indent(&subtract_statement(table, aggregation, "old"), 3));
            lines.push(indent(&add, 3));
            lines.push("   end if;".to_string());
        }
        None => {
            lines.push("   if tg_op = 'UPDATE' then".to_string());
            lines.push(indent(&subtract_statement(table, aggregation, "old"), 3));
            lines.push("   end if;".to_string());
            lines.push(add_new_statement(table, aggregation));
        }
    }
    lines
}

/// Lines for the body of the `after delete` trigger function.
pub fn delete_aggregation_sql(table: &Table, aggregation: &Aggregation) -> Vec<String> {
    vec![subtract_statement(table, aggregation, "old")]
}

fn indent(sql: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    sql.lines()
        .map(|line| format!("{}{}", pad, line))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use crate::common::test_support::make_context;
    use crate::common::trigger_generator::TriggerGenerator;
    use crate::postgresql::postgres_trigger_generator::PostgresTriggerGenerator;
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
        let (ctx, buffer) = make_context(model, DatabaseType::Postgresql);
        PostgresTriggerGenerator::new(ctx).output_triggers();
        buffer.contents()
    }

    #[test]
    fn insert_update_trigger_upserts_into_destination_keyed_on_groups_and_bucket() {
        let sql = generate(None, AggregationFrequency::Monthly);

        assert!(sql.contains("insert into app.daily_sales as dest (updated_at, sold, store_id, total, sale_count)"), "{sql}");
        assert!(
            sql.contains("cast(date_trunc('month', sold) as date) as sold"),
            "{sql}"
        );
        assert!(
            sql.contains("coalesce(amount, 0) as total, 1 as sale_count"),
            "{sql}"
        );
        assert!(
            sql.contains("on conflict (store_id, sold) do update set"),
            "{sql}"
        );
        assert!(
            sql.contains("total = coalesce(dest.total, 0) + excluded.total"),
            "{sql}"
        );
    }

    #[test]
    fn update_only_reaggregates_when_a_contributing_column_changed() {
        let sql = generate(None, AggregationFrequency::Daily);

        assert!(sql.contains("if tg_op = 'INSERT' then"), "{sql}");
        assert!(
            sql.contains("elsif new.amount is distinct from old.amount\n         or new.store_ref is distinct from old.store_ref\n         or new.sold is distinct from old.sold then"),
            "{sql}"
        );
    }

    #[test]
    fn criteria_filters_rows_and_disables_the_changed_column_shortcut() {
        let sql = generate(Some("voided = false"), AggregationFrequency::Daily);

        assert!(
            sql.contains("from (select new.*) as src where (voided = false)"),
            "{sql}"
        );
        assert!(
            sql.contains("from (select old.*) as src where (voided = false)"),
            "{sql}"
        );
        assert!(!sql.contains("is distinct from"), "{sql}");
    }

    #[test]
    fn delete_trigger_subtracts_the_old_row() {
        let sql = generate(None, AggregationFrequency::Yearly);

        assert!(
            sql.contains("create trigger sale_delete after delete on app.sale"),
            "{sql}"
        );
        assert!(sql.contains("update app.daily_sales as dest set\n      total = coalesce(dest.total, 0) - agg.total,\n      sale_count = coalesce(dest.sale_count, 0) - agg.sale_count,\n      updated_at = now()"), "{sql}");
        assert!(
            sql.contains("cast(date_trunc('year', sold) as date) as sold"),
            "{sql}"
        );
        assert!(
            sql.contains("where dest.store_id = agg.store_id and dest.sold = agg.sold;"),
            "{sql}"
        );
    }
}
