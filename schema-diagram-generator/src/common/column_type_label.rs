use schema_model::model::column_type::ColumnType;

pub fn column_type_label(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::Sequence => "int",
        ColumnType::LongSequence => "bigint",
        ColumnType::Byte => "tinyint",
        ColumnType::Short => "smallint",
        ColumnType::Int => "int",
        ColumnType::Long => "bigint",
        ColumnType::Float => "float",
        ColumnType::Double => "double",
        ColumnType::Decimal => "decimal",
        ColumnType::Boolean => "boolean",
        ColumnType::Date => "date",
        ColumnType::DateTime => "datetime",
        ColumnType::Time => "time",
        ColumnType::Timestamp => "timestamp",
        ColumnType::Char => "char",
        ColumnType::Varchar => "varchar",
        ColumnType::Enum => "varchar",
        ColumnType::Text => "text",
        ColumnType::Binary => "binary",
        ColumnType::Uuid => "uuid",
        ColumnType::Json => "json",
        ColumnType::Array => "array",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_column_types_map_correctly() {
        let cases = [
            (ColumnType::Sequence, "int"),
            (ColumnType::LongSequence, "bigint"),
            (ColumnType::Byte, "tinyint"),
            (ColumnType::Short, "smallint"),
            (ColumnType::Int, "int"),
            (ColumnType::Long, "bigint"),
            (ColumnType::Float, "float"),
            (ColumnType::Double, "double"),
            (ColumnType::Decimal, "decimal"),
            (ColumnType::Boolean, "boolean"),
            (ColumnType::Date, "date"),
            (ColumnType::DateTime, "datetime"),
            (ColumnType::Time, "time"),
            (ColumnType::Timestamp, "timestamp"),
            (ColumnType::Char, "char"),
            (ColumnType::Varchar, "varchar"),
            (ColumnType::Enum, "varchar"),
            (ColumnType::Text, "text"),
            (ColumnType::Binary, "binary"),
            (ColumnType::Uuid, "uuid"),
            (ColumnType::Json, "json"),
            (ColumnType::Array, "array"),
        ];
        for (column_type, expected) in cases {
            assert_eq!(column_type_label(column_type), expected, "failed for {column_type:?}");
        }
    }
}
