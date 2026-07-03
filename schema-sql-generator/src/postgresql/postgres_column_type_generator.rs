use crate::common::column_type_generator::ColumnTypeGenerator;
use crate::common::generator_context::GeneratorContext;
use crate::postgresql::postgres_util::to_snake_case;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::schema::Schema;

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

    fn byte_sql(&self) -> String {
        "smallint".to_string()
    }

    fn text_sql(&self, column: &Column) -> String {
        let database_model = self.context.settings().database_model();
        let schema = database_model.find_schema(column.schema_name());
        if !schema.case_sensitive_text() {
            return "citext".to_string();
        }
        "text".to_string()
    }

    fn citext_sql(&self) -> String {
        "citext".to_string()
    }

    fn cstext_sql(&self) -> String {
        "text".to_string()
    }

    fn timestamp_tz_sql(&self) -> String {
        "timestamptz".to_string()
    }

    fn binary_sql(&self) -> String {
        "bytea".to_string()
    }

    fn uuid_default_value_sql(&self, _schema: &Schema) -> String {
        if self.context.settings().target_postgres_version() >= 18 {
            "uuidv7()".to_string()
        } else {
            "generate_uuid()".to_string()
        }
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

    fn varchar_sql(&self, _column: &Column) -> String {
        "text".to_string()
    }

    fn uuid_sql(&self, _column: &Column) -> String {
        "uuid".to_string()
    }

    fn json_sql(&self, _column: &Column) -> String {
        "jsonb".to_string()
    }

    fn enum_sql(&self, column: &Column) -> String {
        let enum_type_opt = column.enum_type();
        let enum_type_name = enum_type_opt.as_ref().unwrap();
        let schema_prefix = if let Some(schema_name) = column.schema_name() {
            format!("{}.", schema_name.to_lowercase())
        } else {
            String::new()
        };

        format!("{}{}", schema_prefix, to_snake_case(enum_type_name))
    }

    fn native_boolean_sql(&self) -> String {
        "boolean".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::generate_options::GenerateOptions;
    use crate::common::generator_context::GeneratorContext;
    use crate::common::print_writer::PrintWriter;
    use crate::common::sql_generator_settings::SqlGeneratorSettings;
    use crate::common::sql_writer::SqlWriter;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_context(model: DatabaseModel) -> (GeneratorContext, TableBuilder) {
        let table = TableBuilder::new(None::<&str>, "test");
        let options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );
        let settings = SqlGeneratorSettings::new(DatabaseType::Postgresql, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let ctx = GeneratorContext::new(settings, writer);
        (ctx, table)
    }

    fn make_model_default() -> DatabaseModel {
        let schema = SchemaBuilder::new(None::<&str>).build();
        DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
    }

    fn assert_type(column_type: ColumnType, expected: &str) {
        let model = make_model_default();
        let (ctx, table_builder) = make_context(model);
        let generator = PostgresColumnTypeGenerator::new(ctx);
        let table = table_builder.build();
        let col = ColumnBuilder::new(None::<&str>, "col", column_type).build();
        assert_eq!(generator.column_type_sql(&table, &col), expected);
    }

    #[test]
    fn sequence_types() {
        assert_type(ColumnType::Sequence, "serial");
        assert_type(ColumnType::LongSequence, "bigserial");
    }

    #[test]
    fn numeric_types() {
        assert_type(ColumnType::Byte, "smallint");
        assert_type(ColumnType::Short, "smallint");
        assert_type(ColumnType::Int, "integer");
        assert_type(ColumnType::Long, "bigint");
        assert_type(ColumnType::Float, "real");
        assert_type(ColumnType::Double, "double precision");
        assert_type(ColumnType::Decimal, "decimal");
    }

    #[test]
    fn temporal_types() {
        assert_type(ColumnType::Date, "date");
        assert_type(ColumnType::DateTime, "timestamp");
        assert_type(ColumnType::Time, "time");
        assert_type(ColumnType::Timestamp, "timestamp");
        assert_type(ColumnType::TimestampTz, "timestamptz");
    }

    #[test]
    fn text_types_case_sensitive_schema() {
        assert_type(ColumnType::Varchar, "text");
        assert_type(ColumnType::Text, "text");
        assert_type(ColumnType::CiText, "citext");
        assert_type(ColumnType::CsText, "text");
        assert_type(ColumnType::Char, "char(0)");
        assert_type(ColumnType::Json, "jsonb");
        assert_type(ColumnType::Uuid, "uuid");
    }

    #[test]
    fn text_type_case_insensitive_schema_returns_citext() {
        let schema = SchemaBuilder::new(None::<&str>).case_sensitive_text(false).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, table_builder) = make_context(model);
        let generator = PostgresColumnTypeGenerator::new(ctx);
        let table = table_builder.build();
        let col = ColumnBuilder::new(None::<&str>, "col", ColumnType::Text).build();
        assert_eq!(generator.column_type_sql(&table, &col), "citext");
    }

    #[test]
    fn other_types() {
        assert_type(ColumnType::Boolean, "boolean");
        assert_type(ColumnType::Binary, "bytea");
    }
}
