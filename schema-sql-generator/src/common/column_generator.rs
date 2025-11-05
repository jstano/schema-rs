use crate::common::column_type_generator::{ColumnTypeGenerator};
use crate::common::generator_context::GeneratorContext;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;
use schema_model::model::types::BooleanMode;

pub trait ColumnGenerator {
    fn column_definitions(&self, table: &Table) -> Vec<String>;

    fn column_sql(&self, table: &Table, column: &Column) -> String;

    fn column_options(&self, table: &Table, column: &Column) -> String;

    fn default_value(&self, table: &Table, column: &Column) -> Option<String>;
}

pub struct DefaultColumnGenerator {
    context: GeneratorContext,
    column_type_generator: Box<dyn ColumnTypeGenerator>,
}

impl DefaultColumnGenerator {
    pub fn new(
        context: GeneratorContext,
        column_type_generator: Box<dyn ColumnTypeGenerator>,
    ) -> Self {
        Self {
            column_type_generator,
            context,
        }
    }

    fn convert_boolean_default_constraint(&self, value: bool) -> String {
        match self.context.settings().boolean_mode() {
            BooleanMode::Native => {
                if value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            BooleanMode::YesNo => {
                if value {
                    "'Yes'".to_string()
                } else {
                    "'No'".to_string()
                }
            }
            BooleanMode::YN => {
                if value {
                    "'Y'".to_string()
                } else {
                    "'N'".to_string()
                }
            }
        }
    }

    fn boolean_default_value(&self, default_constraint: Option<&str>) -> Option<String> {
        if default_constraint.is_some() {
            if default_constraint.unwrap().to_ascii_lowercase() == "null" {
                return None;
            }

            let value = matches!(
                default_constraint.unwrap().to_ascii_lowercase().as_str(),
                "true"
            );
            return Some(self.convert_boolean_default_constraint(value));
        }

        Some(self.convert_boolean_default_constraint(false))
    }

    fn uuid_default_value(
        &self,
        table: &Table,
        column: &Column,
        default_constraint: Option<&str>,
    ) -> Option<String> {
        let schema = self
            .context
            .settings()
            .database_model()
            .find_schema(table.schema_name());
        let primary_key_columns = table.primary_key_columns();

        if column.required()
            && primary_key_columns.is_some()
            && primary_key_columns
                .unwrap()
                .contains(&column.name().to_string())
            && table.column_relation(column).is_none()
        {
            return Some(self.column_type_generator.uuid_default_value_sql(schema));
        }

        if default_constraint.is_some()
            && default_constraint.unwrap().to_ascii_lowercase() == "generate_uuid()"
        {
            return Some(self.column_type_generator.uuid_default_value_sql(schema));
        }

        None
    }
}

impl ColumnGenerator for DefaultColumnGenerator {
    fn column_definitions(&self, table: &Table) -> Vec<String> {
        table
            .columns()
            .iter()
            .map(|column| self.column_sql(table, column))
            .collect()
    }

    fn column_sql(&self, table: &Table, column: &Column) -> String {
        let column_options = self.column_options(table, column);

        if column_options.is_empty() {
            return format!(
                "   {} {}",
                column.name(),
                self.column_type_generator.column_type_sql(table, column)
            );
        }

        format!(
            "   {} {} {}",
            column.name(),
            self.column_type_generator.column_type_sql(table, column),
            column_options
        )
    }

    fn column_options(&self, table: &Table, column: &Column) -> String {
        "".to_string()
    }

    fn default_value(&self, table: &Table, column: &Column) -> Option<String> {
        let default_constraint = column.default_constraint();

        match column.column_type() {
            ColumnType::Boolean => self.boolean_default_value(default_constraint),
            ColumnType::Uuid => self.uuid_default_value(table, column, default_constraint),
            _ => default_constraint.map(String::from),
        }
    }
}
