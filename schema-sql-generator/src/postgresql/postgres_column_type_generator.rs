use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::schema::Schema;
use crate::common::column_type_generator::{ColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct PostgresColumnTypeGenerator {
    context: GeneratorContext
}

impl PostgresColumnTypeGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ColumnTypeGenerator for PostgresColumnTypeGenerator {
    fn context(&self) -> &GeneratorContext {
        &self.context
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

    fn uuid_default_value_sql(&self, _schema: &Schema) -> String {
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

    fn varchar_sql(&self, column: &Column) -> String {
        if column.ignore_case() {
            return "citext".to_string();
        }

        "text".to_string()
    }

    fn uuid_sql(&self, _column: &Column) -> String {
        "uuid".to_string()
    }

    fn json_sql(&self, _column: &Column) -> String {
        "jsonb".to_string()
    }

    fn native_boolean_sql(&self) -> String {
        "boolean".to_string()
    }
}
