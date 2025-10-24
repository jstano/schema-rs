use schema_model::model::column::Column;
use schema_model::model::table::Table;

pub struct ColumnGenerator {
}

impl ColumnGenerator {
}

trait ColumnGeneratorTrait {
    fn get_column_sql(&self, table: &Table, column: &Column) -> String {
        let column_options = self.get_column_options(table, column);

        /*
        if (!is_blank(&column_options)) {
            return format!("   %s %s %s", column.name(), getColumnTypeGenerator().getColumnTypeSql(table, column), columnOptions);
        }

        format!("   %s %s", column.getName(), getColumnTypeGenerator().getColumnTypeSql(table, column))
        */
        "".to_string()
    }

    fn get_column_options(&self, table: &Table, column: &Column) -> String;

    fn get_default_value(&self, table: &Table, column: &Column) -> String;
}
