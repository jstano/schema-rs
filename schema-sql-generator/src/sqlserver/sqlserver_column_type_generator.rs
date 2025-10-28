use schema_model::model::column::Column;
use schema_model::model::schema::Schema;
use schema_model::model::table::Table;
use crate::common::column_type_generator::{ColumnTypeGenerator, DefaultColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;

pub struct SqlServerColumnTypeGenerator {
    column_type_generator: DefaultColumnTypeGenerator,
}

impl SqlServerColumnTypeGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_type_generator: DefaultColumnTypeGenerator::new(context),
        }
    }
}

impl ColumnTypeGenerator for SqlServerColumnTypeGenerator {
    fn column_type_sql(&self, table: &Table, column: &Column) -> String {
        self.column_type_generator.column_type_sql(table, column)
    }

    fn sequence_sql(&self) -> String {
        self.column_type_generator.sequence_sql()
    }

    fn long_sequence_sql(&self) -> String {
        self.column_type_generator.long_sequence_sql()
    }

    fn text_sql(&self, column: &Column) -> String {
        self.column_type_generator.text_sql(column)
    }

    fn binary_sql(&self) -> String {
        self.column_type_generator.binary_sql()
    }

    fn uuid_default_value_sql(&self, schema: &Schema) -> String {
        self.column_type_generator.uuid_default_value_sql(schema)
    }

    fn array_sql(&self, column: &Column) -> String {
        self.column_type_generator.array_sql(column)
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
