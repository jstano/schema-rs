use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::schema::Schema;
use schema_model::model::table::Table;
use crate::common::column_type_generator::{ColumnTypeGenerator, DefaultColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct PostgresColumnTypeGenerator {
    column_type_generator: DefaultColumnTypeGenerator,
}

impl PostgresColumnTypeGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_type_generator: DefaultColumnTypeGenerator::new(context),
        }
    }
}

impl ColumnTypeGenerator for PostgresColumnTypeGenerator {
    fn column_type_sql(&self, table: &Table, column: &Column) -> String {
        match column.column_type() {
            ColumnType::Sequence => self.sequence_sql(),
            ColumnType::LongSequence => self.long_sequence_sql(),
            ColumnType::Text => self.text_sql(column),
            ColumnType::Binary => self.binary_sql(),
            ColumnType::Array => self.array_sql(column),
            _ => self.column_type_generator.column_type_sql(table, column)
        }
    }

    fn sequence_sql(&self) -> String {
        "serial".to_string()
    }

    fn long_sequence_sql(&self) -> String {
        "bigserial".to_string()
    }

    fn text_sql(&self, column: &Column) -> String {
        if column.ignore_case() {
            return "citext".to_string();
        }

        "text".to_string()
    }

    fn binary_sql(&self) -> String {
        "bytea".to_string()
    }

    fn uuid_default_value_sql(&self, schema: &Schema) -> String {
        "generate_uuid()".to_string()
    }

    fn array_sql(&self, column: &Column) -> String {
        match column.column_type() {
            ColumnType::Byte => {self.byte_sql() + "[]"}
            ColumnType::Short => {self.short_sql() + "[]"}
            ColumnType::Int => {self.int_sql() + "[]"}
            ColumnType::Long => {self.long_sql() + "[]"}
            ColumnType::Decimal => {self.decimal_sql(column) + "[]"}
            ColumnType::Char => {self.char_sql(column) + "[]"}
            ColumnType::Varchar => {self.varchar_sql(column) + "[]"}
            ColumnType::Text => {self.text_sql(column) + "[]"}
            other => panic!("Unsupported array type: {:?}", other)
        }
    }

    fn byte_sql(&self) -> String {
        self.column_type_generator.byte_sql()
    }

    fn short_sql(&self) -> String {
        self.column_type_generator.short_sql()
    }

    fn int_sql(&self) -> String {
        self.column_type_generator.int_sql()
    }

    fn long_sql(&self) -> String {
        self.column_type_generator.long_sql()
    }

    fn float_sql(&self) -> String {
        self.column_type_generator.float_sql()
    }

    fn double_sql(&self) -> String {
        self.column_type_generator.double_sql()
    }

    fn decimal_sql(&self, column: &Column) -> String {
        self.column_type_generator.decimal_sql(column)
    }

    fn boolean_sql(&self) -> String {
        self.column_type_generator.boolean_sql()
    }

    fn date_sql(&self) -> String {
        self.column_type_generator.date_sql()
    }

    fn date_time_sql(&self) -> String {
        self.column_type_generator.date_time_sql()
    }

    fn time_sql(&self) -> String {
        self.column_type_generator.time_sql()
    }

    fn char_sql(&self, column: &Column) -> String {
        self.column_type_generator.char_sql(column)
    }

    fn varchar_sql(&self, column: &Column) -> String {
        self.column_type_generator.varchar_sql(column)
    }

    fn uuid_sql(&self, column: &Column) -> String {
        self.column_type_generator.uuid_sql(column)
    }

    fn json_sql(&self, column: &Column) -> String {
        self.column_type_generator.json_sql(column)
    }

    fn enum_sql(&self, column: &Column) -> String {
        self.column_type_generator.enum_sql(column)
    }

    fn native_boolean_sql(&self) -> String {
        self.column_type_generator.native_boolean_sql()
    }
}
