use crate::common::generator_context::GeneratorContext;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;
use schema_model::model::types::BooleanMode;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};

const CK_PREFIX: &str = "ck_";

pub trait ColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultColumnConstraintGenerator {
    context: GeneratorContext,
}

impl DefaultColumnConstraintGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn generate_constraint(&self, table: &Table, column: &Column) -> String {
        let constraint_sql = self.check_constraint_sql(column);

        if constraint_sql.is_some() {
            return format!(
                "   constraint {} {}",
                self.constraint_name(table.name(), column.name()),
                constraint_sql.unwrap()
            );
        }

        String::new()
    }

    fn constraint_name(&self, table_name: &str, column_name: &str) -> String {
        let table_name = table_name.to_lowercase();
        let column_name = column_name.to_lowercase();
        let hash = self.combined_hash(&table_name, &column_name);
        let table_name = self.truncate_lower(&table_name, 9);
        let column_name = self.truncate_lower(&column_name, 9);

        format!("{}{}_{}_{}", CK_PREFIX, table_name, column_name, hash)
    }

    fn truncate_lower(&self, s: &str, max_len: usize) -> String {
        s.to_lowercase().chars().take(max_len).collect()
    }

    fn combined_hash(&self, table_name: &str, column_name: &str) -> String {
        let combined_name = format!("{}_{}", table_name, column_name);
        let mut hasher = DefaultHasher::new();
        combined_name.hash(&mut hasher);
        format!("{:X}", hasher.finish())
    }

    fn check_constraint_sql(&self, column: &Column) -> Option<String> {
        if column.column_type() == ColumnType::Boolean {
            self.boolean_check_constraint(column)
        } else if let Some(constraint) = column.check_constraint() {
            Some(constraint.to_string())
        } else if column.column_type() == ColumnType::Enum {
            self.enum_check_constraint_sql(column)
        } else if column.has_min_or_max_value() {
            self.min_max_constraint_sql(column)
        } else {
            None
        }
    }

    fn boolean_check_constraint(&self, column: &Column) -> Option<String> {
        match self.context.settings().boolean_mode() {
            BooleanMode::YesNo => Some(format!("check({} in ('Yes','No'))", column.name())),
            BooleanMode::YN => Some(format!("check({} in ('Y','N'))", column.name())),
            BooleanMode::Native => None,
        }
    }

    fn enum_check_constraint_sql(&self, column: &Column) -> Option<String> {
        let schema_name = column.schema_name();
        let schema = self
            .context
            .settings()
            .database_model()
            .find_schema(schema_name);
        let enum_type = column.enum_type();
        let enum_values = schema.get_enum_type(enum_type?).values().clone();

        let joined_values = enum_values
            .iter()
            .map(|value| format!("'{}'", value.code()))
            .collect::<Vec<_>>()
            .join(", ");

        Some(format!("check({} in ({}))", column.name(), joined_values))
    }

    fn min_max_constraint_sql(&self, column: &Column) -> Option<String> {
        let min_value = column.min_value();
        let max_value = column.max_value();
        let mut sql = String::from("check(");

        if min_value.is_some() {
            sql.push_str(column.name());
            sql.push_str(" >= ");
            sql.push_str(min_value.unwrap().to_string().as_str());
        }

        if min_value.is_some() && max_value.is_some() {
            sql.push_str(" and ");
        }

        if max_value.is_some() {
            sql.push_str(column.name());
            sql.push_str(" <= ");
            sql.push_str(max_value.unwrap().to_string().as_str());
        }

        sql.push_str(")");

        Some(sql)
    }
}

impl ColumnConstraintGenerator for DefaultColumnConstraintGenerator {
    fn column_check_constraints(&self, table: &Table) -> Vec<String> {
        let columns = table.columns_with_check_constraints(self.context.settings().boolean_mode());

        columns
            .iter()
            .map(|column| self.generate_constraint(table, column))
            .collect()
    }
}
