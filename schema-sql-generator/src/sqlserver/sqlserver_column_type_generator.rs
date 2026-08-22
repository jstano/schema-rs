use crate::common::column_type_generator::ColumnTypeGenerator;
use crate::common::generator_context::GeneratorContext;
use schema_model::model::column::Column;
use schema_model::model::enum_type::EnumType;
use schema_model::model::schema::Schema;
use schema_model::model::types::BooleanMode;
use std::cmp;

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

    fn text_sql(&self, _column: &Column) -> String {
        "nvarchar(max)".to_string()
    }

    fn citext_sql(&self) -> String {
        "nvarchar(max)".to_string()
    }

    fn cstext_sql(&self) -> String {
        "nvarchar(max)".to_string()
    }

    fn timestamp_tz_sql(&self) -> String {
        "datetimeoffset".to_string()
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

    fn date_sql(&self) -> String {
        "datetime".to_string()
    }

    fn date_time_sql(&self) -> String {
        "datetime".to_string()
    }

    fn time_sql(&self) -> String {
        "datetime".to_string()
    }

    fn char_sql(&self, column: &Column) -> String {
        let length = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };
        format!("nchar({})", length).to_string()
    }

    fn varchar_sql(&self, column: &Column) -> String {
        let length = if column.length() == -1 { "max".to_string() } else { column.length().to_string() };
        format!("nvarchar({})", length).to_string()
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

    fn native_boolean_literal(&self, value: bool) -> String {
        if value {
            "1".to_string()
        } else {
            "0".to_string()
        }
    }

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
            return format!("nvarchar({})", max_length);
        }

        format!("nchar({})", max_length)
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
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_context() -> (GeneratorContext, TableBuilder) {
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let table = TableBuilder::new(None::<&str>, "test");
        let options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );
        let settings = SqlGeneratorSettings::new(DatabaseType::SqlServer, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let ctx = GeneratorContext::new(settings, writer);
        (ctx, table)
    }

    fn assert_type(column_type: ColumnType, expected: &str) {
        let (ctx, table_builder) = make_context();
        let generator = SqlServerColumnTypeGenerator::new(ctx);
        let table = table_builder.build();
        let col = ColumnBuilder::new(None::<&str>, "col", column_type).build();
        assert_eq!(generator.column_type_sql(&table, &col), expected);
    }

    #[test]
    fn sequence_types() {
        assert_type(ColumnType::Sequence, "integer identity(1,1)");
        assert_type(ColumnType::LongSequence, "bigint identity(1,1)");
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
        assert_type(ColumnType::Date, "datetime");
        assert_type(ColumnType::DateTime, "datetime");
        assert_type(ColumnType::Time, "datetime");
        assert_type(ColumnType::Timestamp, "datetime");
        assert_type(ColumnType::TimestampTz, "datetimeoffset");
    }

    #[test]
    fn text_types() {
        assert_type(ColumnType::Text, "nvarchar(max)");
        assert_type(ColumnType::CiText, "nvarchar(max)");
        assert_type(ColumnType::CsText, "nvarchar(max)");
        assert_type(ColumnType::Json, "json");
        assert_type(ColumnType::Uuid, "uniqueidentifier");
    }

    #[test]
    fn other_types() {
        assert_type(ColumnType::Boolean, "bit");
        assert_type(ColumnType::Binary, "varbinary(max)");
    }

    #[test]
    fn native_boolean_literal_uses_bit_values() {
        let (ctx, _table_builder) = make_context();
        let generator = SqlServerColumnTypeGenerator::new(ctx);

        assert_eq!(generator.native_boolean_literal(true), "1");
        assert_eq!(generator.native_boolean_literal(false), "0");
    }

    #[test]
    fn enum_sql_uses_nvarchar_when_values_have_different_lengths() {
        use schema_model::model::enum_type::{EnumType, EnumValue};

        let enum_type = EnumType::new(
            "status_type",
            vec![
                EnumValue::new("SHORT", Some("A".to_string())),
                EnumValue::new("LONGER", Some("ABC".to_string())),
            ],
        );
        let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );
        let settings = SqlGeneratorSettings::new(DatabaseType::SqlServer, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let ctx = GeneratorContext::new(settings, writer);

        let generator = SqlServerColumnTypeGenerator::new(ctx);
        let table = TableBuilder::new(None::<&str>, "accounts").build();
        let col = ColumnBuilder::new(None::<&str>, "status", ColumnType::Enum)
            .enum_type(Some("status_type".to_string()))
            .build();

        assert_eq!(generator.column_type_sql(&table, &col), "nvarchar(3)");
    }

    #[test]
    fn enum_sql_uses_nchar_when_all_values_share_length() {
        use schema_model::model::enum_type::{EnumType, EnumValue};

        let enum_type = EnumType::new(
            "gender_type",
            vec![
                EnumValue::new("MALE", Some("M".to_string())),
                EnumValue::new("FEMALE", Some("F".to_string())),
            ],
        );
        let schema = SchemaBuilder::new(None::<&str>).add_enum_type(enum_type).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );
        let settings = SqlGeneratorSettings::new(DatabaseType::SqlServer, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let ctx = GeneratorContext::new(settings, writer);

        let generator = SqlServerColumnTypeGenerator::new(ctx);
        let table = TableBuilder::new(None::<&str>, "accounts").build();
        let col = ColumnBuilder::new(None::<&str>, "gender", ColumnType::Enum)
            .enum_type(Some("gender_type".to_string()))
            .build();

        assert_eq!(generator.column_type_sql(&table, &col), "nchar(1)");
    }
}
