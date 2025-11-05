use std::cmp;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::enum_type::EnumType;
use schema_model::model::schema::Schema;
use schema_model::model::table::Table;
use schema_model::model::types::BooleanMode;
use crate::common::generator_context::GeneratorContext;

pub trait ColumnTypeGenerator {
    fn context(&self) -> &GeneratorContext;

    fn column_type_sql(&self, table: &Table, column: &Column) -> String {
        match column.column_type() {
            ColumnType::Sequence => self.sequence_sql(),
            ColumnType::LongSequence => self.long_sequence_sql(),
            ColumnType::Byte => self.byte_sql(),
            ColumnType::Short => self.short_sql(),
            ColumnType::Int => self.int_sql(),
            ColumnType::Long => self.long_sql(),
            ColumnType::Float => self.float_sql(),
            ColumnType::Double => self.double_sql(),
            ColumnType::Decimal => self.decimal_sql(column),
            ColumnType::Boolean => self.boolean_sql(),
            ColumnType::Date => self.date_sql(),
            ColumnType::DateTime => self.date_time_sql(),
            ColumnType::Time => self.time_sql(),
            ColumnType::Timestamp => self.date_time_sql(),
            ColumnType::Char => self.char_sql(column),
            ColumnType::Varchar => self.varchar_sql(column),
            ColumnType::Enum => self.enum_sql(column),
            ColumnType::Text => self.text_sql(column),
            ColumnType::Binary => self.binary_sql(),
            ColumnType::Uuid => self.uuid_sql(column),
            ColumnType::Json => self.json_sql(column),
            ColumnType::Array => self.array_sql(column),
        }
    }

    fn sequence_sql(&self) -> String;

    fn long_sequence_sql(&self) -> String;

    fn text_sql(&self, column: &Column) -> String;

    fn binary_sql(&self) -> String;

    fn uuid_default_value_sql(&self, schema: &Schema) -> String;

    fn array_sql(&self, column: &Column) -> String;

    fn byte_sql(&self) -> String {
        "tinyint".to_string()
    }

    fn short_sql(&self) -> String {
        "smallint".to_string()
    }

    fn int_sql(&self) -> String {
        "integer".to_string()
    }

    fn long_sql(&self) -> String {
        "bigint".to_string()
    }

    fn float_sql(&self) -> String {
        "real".to_string()
    }

    fn double_sql(&self) -> String {
        "double precision".to_string()
    }

    fn decimal_sql(&self, column: &Column) -> String {
        let length = column.length();
        let scale = column.scale();

        if length == 0 && scale == 0 {
            return "decimal".to_string();
        }

        if scale == 0 {
            return format!("decimal({})", length);
        }

        format!("decimal({},{})", length, scale)
    }

    fn boolean_sql(&self) -> String {
        match self.context().settings().boolean_mode() {
            BooleanMode::Native => self.native_boolean_sql(),
            BooleanMode::YesNo => "varchar(3)".to_string(),
            BooleanMode::YN => "char(1)".to_string()
        }
    }

    fn date_sql(&self) -> String {
        "date".to_string()
    }

    fn date_time_sql(&self) -> String {
        "timestamp".to_string()
    }

    fn time_sql(&self) -> String {
        "time".to_string()
    }

    fn char_sql(&self, column: &Column) -> String {
        format!("char({})", column.length())
    }

    fn varchar_sql(&self, column: &Column) -> String {
        format!("varchar({})", column.length())
    }

    fn uuid_sql(&self, column: &Column) -> String {
        "varchar(36)".to_string()
    }

    fn json_sql(&self, column: &Column) -> String;

    fn enum_sql(&self, column: &Column) -> String {
        let database_model = self.context().settings().database_model();
        let enum_type: &EnumType = database_model.find_enum_type(column.schema_name(), column.enum_type().as_ref().unwrap());

        let mut min_length = usize::MAX;
        let mut max_length = 0;

        enum_type.values().iter().for_each(|enum_value| {
            let code = enum_value.code();

            min_length = cmp::min(min_length, code.len());
            max_length = cmp::max(max_length, code.len());
        });

        if min_length != max_length {
            return format!("varchar({})", max_length);
        }

        format!("char({})", max_length)
    }

    fn native_boolean_sql(&self) -> String;
}
