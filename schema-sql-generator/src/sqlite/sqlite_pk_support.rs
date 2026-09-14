use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;

/// SQLite has no `AUTO_INCREMENT`; the only way to get an auto-incrementing key is the
/// `INTEGER PRIMARY KEY AUTOINCREMENT` rowid-alias form, and it must be declared inline on
/// the column rather than as a separate table-level `PRIMARY KEY (...)` constraint - a table
/// with both is rejected ("table has more than one primary key declared"). This identifies
/// the column that qualifies: a `Sequence`/`LongSequence` column that is the sole column of
/// the table's primary key.
pub(crate) fn inline_autoincrement_pk_column(table: &Table) -> Option<&Column> {
    let columns = table.primary_key_columns()?;

    if columns.len() != 1 || !table.has_column(&columns[0]) {
        return None;
    }

    let column = table.column(&columns[0]);

    matches!(column.column_type(), ColumnType::Sequence | ColumnType::LongSequence).then_some(column)
}
