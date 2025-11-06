use schema_model::model::column::Column;
use schema_model::model::schema::Schema;
use schema_model::model::types::BooleanMode;
use crate::common::column_type_generator::{ColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqlServerColumnTypeGenerator {
    context: GeneratorContext
}

impl SqlServerColumnTypeGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context
        }
    }
}

impl ColumnTypeGenerator for SqlServerColumnTypeGenerator {
    fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn sequence_sql(&self) -> String {
        "integer identity(1,1)".to_string()
    }

    fn long_sequence_sql(&self) -> String {
        "bigint identity(1,1)".to_string()
    }

    fn text_sql(&self, column: &Column) -> String {
        let length = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };

        if column.unicode() {
            format!("nvarchar({})", length).to_string()
        } else {
            format!("varchar({})", length).to_string()
        }
    }

    fn binary_sql(&self) -> String {
        "varbinary(max)".to_string()
    }

    fn uuid_default_value_sql(&self, _schema: &Schema) -> String {
        "newid()".to_string()
    }

    fn array_sql(&self, _column: &Column) -> String {
        panic!("Sql Server does not support arrays")
    }

    fn boolean_sql(&self) -> String {
        match self.context.settings().boolean_mode() {
            BooleanMode::YesNo  => "nvarchar(3)".to_string(),
            BooleanMode::YN => "nchar(1)".to_string(),
            BooleanMode::Native => "bit".to_string()
        }
    }

    fn date_time_sql(&self) -> String {
        "datetime".to_string()
    }

    fn char_sql(&self, column: &Column) -> String {
        let length = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };

        if column.unicode() {
            format!("nchar({})", length).to_string()
        } else {
            format!("char({})", length).to_string()
        }
    }

    fn varchar_sql(&self, column: &Column) -> String {
        let length = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };

        if column.unicode() {
            format!("nvarchar({})", length).to_string()
        } else {
            format!("varchar({})", length).to_string()
        }
    }

    fn uuid_sql(&self, _column: &Column) -> String {
        "uniqueidentifier".to_string()
    }

    fn json_sql(&self, _column: &Column) -> String {
        "json".to_string()
    }


    fn native_boolean_sql(&self) -> String {
        "bit".to_string()
    }
}
