use crate::common::constraint_naming;
use crate::common::generator_context::GeneratorContext;
use crate::common::sql_string::escape_sql_literal;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;
use schema_model::model::types::BooleanMode;

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

    pub fn generate_constraint(&self, table: &Table, column: &Column) -> String {
        let constraint_sql = self.check_constraint_sql(column);

        if let Some(constraint_sql) = constraint_sql {
            return format!(
                "   constraint {} {}",
                self.constraint_name(table.name(), column.name()),
                constraint_sql
            );
        }

        String::new()
    }

    fn constraint_name(&self, table_name: &str, column_name: &str) -> String {
        constraint_naming::hashed_constraint_name(CK_PREFIX, table_name, column_name)
    }

    fn check_constraint_sql(&self, column: &Column) -> Option<String> {
        if column.column_type() == ColumnType::Boolean {
            self.boolean_check_constraint(column)
        } else if let Some(constraint) = column.check_constraint() {
            Some(Self::wrap_in_check(constraint))
        } else if column.column_type() == ColumnType::Enum {
            self.enum_check_constraint_sql(column)
        } else if column.has_min_or_max_value() {
            self.min_max_constraint_sql(column)
        } else {
            None
        }
    }

    /// The `<check>` element holds just the boolean expression (e.g. `foo = 'ABC'`), not a
    /// full `check(...)` clause - wrap it, unless the user already wrote the `check(...)`
    /// wrapper themselves, in which case wrapping again would double it.
    fn wrap_in_check(expression: &str) -> String {
        let trimmed = expression.trim();

        if trimmed.to_ascii_lowercase().starts_with("check") {
            trimmed.to_string()
        } else {
            format!("check({})", trimmed)
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
            .map(|value| format!("'{}'", escape_sql_literal(value.code())))
            .collect::<Vec<_>>()
            .join(",");

        Some(format!("check({} in ({}))", column.name(), joined_values))
    }

    fn min_max_constraint_sql(&self, column: &Column) -> Option<String> {
        let min_value = column.min_value();
        let max_value = column.max_value();
        let mut sql = String::from("check(");

        if let Some(min_value) = min_value {
            sql.push_str(column.name());
            sql.push_str(" >= ");
            sql.push_str(min_value.to_string().as_str());
        }

        if min_value.is_some() && max_value.is_some() {
            sql.push_str(" and ");
        }

        if let Some(max_value) = max_value {
            sql.push_str(column.name());
            sql.push_str(" <= ");
            sql.push_str(max_value.to_string().as_str());
        }

        sql.push(')');

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn user_check_constraint_is_wrapped_in_check() {
        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "varcharWithCheck", ColumnType::Varchar)
                    .length(10)
                    .check_constraint(Some("varcharWithCheck = 'ABC123'".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(varcharWithCheck = 'ABC123')"));
    }

    #[test]
    fn user_check_constraint_already_wrapped_is_not_double_wrapped() {
        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "code", ColumnType::Varchar)
                    .length(10)
                    .check_constraint(Some("check(code <> '')".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(code <> '')"));
        assert!(!constraints[0].contains("check(check("));
    }
}
