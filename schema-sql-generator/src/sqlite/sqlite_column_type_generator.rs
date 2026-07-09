use crate::common::column_type_generator::ColumnTypeGenerator;
use crate::common::generator_context::GeneratorContext;
use schema_model::model::column::Column;
use schema_model::model::schema::Schema;

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

    fn text_sql(&self, _column: &Column) -> String {
        "text".to_string()
    }

    fn citext_sql(&self) -> String {
        "text".to_string()
    }

    fn cstext_sql(&self) -> String {
        "text".to_string()
    }

    fn binary_sql(&self) -> String {
        "blob".to_string()
    }

    fn uuid_default_value_sql(&self, _schema: &Schema) -> String {
        "uuidv4()".to_string()
    }

    fn array_sql(&self, _column: &Column) -> String {
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

    fn uuid_sql(&self, _column: &Column) -> String {
        "text".to_string()
    }

    fn json_sql(&self, _column: &Column) -> String {
        "text".to_string()
    }

    fn native_boolean_sql(&self) -> String {
        "boolean".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::column_type_generator::ColumnTypeGenerator;
    use crate::common::generate_options::GenerateOptions;
    use crate::common::generator_context::GeneratorContext;
    use crate::common::print_writer::PrintWriter;
    use crate::common::sql_generator_settings::SqlGeneratorSettings;
    use crate::common::sql_writer::SqlWriter;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
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
        let settings = SqlGeneratorSettings::new(DatabaseType::Sqlite, &options);
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
        let generator = SqliteColumnTypeGenerator::new(ctx);
        let table = table_builder.build();
        let col = ColumnBuilder::new(None::<&str>, "col", column_type).build();
        assert_eq!(generator.column_type_sql(&table, &col), expected);
    }

    #[test]
    fn sequence_types() {
        assert_type(ColumnType::Sequence, "integer auto_increment");
        assert_type(ColumnType::LongSequence, "integer auto_increment");
    }

    #[test]
    fn numeric_types() {
        assert_type(ColumnType::Byte, "tinyint");
        assert_type(ColumnType::Short, "smallint");
        assert_type(ColumnType::Int, "integer");
        assert_type(ColumnType::Long, "bigint");
        assert_type(ColumnType::Float, "real");
        assert_type(ColumnType::Double, "double precision");
        assert_type(ColumnType::Decimal, "decimal");
    }

    #[test]
    fn temporal_types() {
        assert_type(ColumnType::Date, "text");
        assert_type(ColumnType::DateTime, "text");
        assert_type(ColumnType::Time, "text");
        assert_type(ColumnType::Timestamp, "text");
    }

    #[test]
    fn text_types() {
        assert_type(ColumnType::Varchar, "varchar(0)");
        assert_type(ColumnType::Text, "text");
        assert_type(ColumnType::CiText, "text");
        assert_type(ColumnType::CsText, "text");
        assert_type(ColumnType::Char, "char(0)");
        assert_type(ColumnType::Json, "text");
        assert_type(ColumnType::Uuid, "text");
    }

    #[test]
    fn other_types() {
        assert_type(ColumnType::Boolean, "boolean");
        assert_type(ColumnType::Binary, "blob");
    }

    #[test]
    fn array_sql_panics() {
        let model = make_model_default();
        let (ctx, table_builder) = make_context(model);
        let generator = SqliteColumnTypeGenerator::new(ctx);
        let table = table_builder.build();
        let col = ColumnBuilder::new(None::<&str>, "col", ColumnType::Array).build();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generator.column_type_sql(&table, &col)
        }));
        assert!(result.is_err());
    }
}
