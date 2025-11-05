use schema_model::model::column::Column;
use schema_model::model::schema::Schema;
use crate::common::column_type_generator::{ColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqliteColumnTypeGenerator {
    context: GeneratorContext
}

impl SqliteColumnTypeGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context
        }
    }
}

impl ColumnTypeGenerator for SqliteColumnTypeGenerator {
    fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn sequence_sql(&self) -> String {
        "integer auto_increment".to_string()
    }

    fn long_sequence_sql(&self) -> String {
        "integer auto_increment".to_string()
    }

    fn text_sql(&self, column: &Column) -> String {
        "text".to_string()
    }

    fn binary_sql(&self) -> String {
        "blob".to_string()
    }

    fn uuid_default_value_sql(&self, schema: &Schema) -> String {
        "uuidv4()".to_string()
    }

    fn array_sql(&self, column: &Column) -> String {
        panic!("SQLite does not support arrays")
    }

    fn date_sql(&self) -> String {
        "text".to_string()
    }

    fn date_time_sql(&self) -> String {
        "text".to_string()
    }

    fn time_sql(&self) -> String {
        "text".to_string()
    }

    fn uuid_sql(&self, column: &Column) -> String {
        "text".to_string()
    }

    fn json_sql(&self, column: &Column) -> String {
        "text".to_string()
    }

    fn native_boolean_sql(&self) -> String {
        "boolean".to_string()
    }
}
