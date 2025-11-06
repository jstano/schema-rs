use schema_model::model::column::Column;
use schema_model::model::schema::Schema;
use crate::common::column_type_generator::{ColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct MySqlColumnTypeGenerator {
    context: GeneratorContext
}

impl MySqlColumnTypeGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }
}

impl ColumnTypeGenerator for MySqlColumnTypeGenerator {
    fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn sequence_sql(&self) -> String {
        "integer auto_increment".to_string()
    }

    fn long_sequence_sql(&self) -> String {
        "bigint auto_increment".to_string()
    }

    fn text_sql(&self, _column: &Column) -> String {
        "mediumtext".to_string()
    }

    fn binary_sql(&self) -> String {
        "mediumblob".to_string()
    }

    fn uuid_default_value_sql(&self, _schema: &Schema) -> String {
        "uuid()".to_string()
    }

    fn array_sql(&self, _column: &Column) -> String {
        panic!("MySQL does not support arrays")
    }

    fn uuid_sql(&self, _column: &Column) -> String {
        "char(36)".to_string()
    }

    fn json_sql(&self, _column: &Column) -> String {
        "mediumtext".to_string()
    }

    fn native_boolean_sql(&self) -> String {
        "boolean".to_string()
    }
}
