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

    /// A column's boolean/user-check/enum/min-max constraints are independent attributes,
    /// not alternatives (`Table::needs_check_constraints` already OR's them together) - so
    /// every applicable one is collected and AND'ed into a single `check(...)` clause
    /// rather than picking just the first that applies, since the hashed constraint name
    /// is per (table, column) and can't hold more than one named constraint anyway (M3).
    pub fn check_constraint_sql(&self, column: &Column) -> Option<String> {
        let mut clauses = Vec::new();

        if column.column_type() == ColumnType::Boolean
            && let Some(clause) = self.boolean_check_clause(column)
        {
            clauses.push(clause);
        }
        if let Some(constraint) = column.check_constraint() {
            clauses.push(Self::strip_check_wrapper(constraint));
        }
        if column.column_type() == ColumnType::Enum
            && let Some(clause) = self.enum_check_clause(column)
        {
            clauses.push(clause);
        }
        if column.has_min_or_max_value() {
            clauses.push(self.min_max_check_clause(column));
        }

        match clauses.len() {
            0 => None,
            // A lone clause is rendered exactly as it always was (including a
            // user-supplied clause that was already fully formed), so existing output
            // doesn't gain redundant parentheses in the common single-constraint case.
            1 => Some(format!("check({})", clauses.remove(0))),
            _ => Some(format!(
                "check({})",
                clauses
                    .iter()
                    .map(|clause| format!("({})", clause))
                    .collect::<Vec<_>>()
                    .join(" and ")
            )),
        }
    }

    /// The `<check>` element holds just the boolean expression (e.g. `foo = 'ABC'`), not a
    /// full `check(...)` clause, but a user may have written the `check(...)` wrapper
    /// themselves anyway - strip it back off so every clause this generator combines is a
    /// bare predicate, regardless of which form the user wrote it in.
    fn strip_check_wrapper(expression: &str) -> String {
        let trimmed = expression.trim();

        if let Some(rest) = trimmed
            .get(5..)
            .filter(|_| trimmed.to_ascii_lowercase().starts_with("check"))
        {
            let rest = rest.trim();
            // Only a wrapper if the opening paren's partner is the very last character:
            // `check (a > 1) or (b < 2)` starts with `(` and ends with `)` but is not wrapped.
            if rest.starts_with('(') && closing_paren_index(rest) == Some(rest.len() - 1) {
                return rest[1..rest.len() - 1].to_string();
            }
        }

        trimmed.to_string()
    }

    fn boolean_check_clause(&self, column: &Column) -> Option<String> {
        match self.context.settings().boolean_mode() {
            BooleanMode::YesNo => Some(format!("{} in ('Yes','No')", column.name())),
            BooleanMode::YN => Some(format!("{} in ('Y','N')", column.name())),
            BooleanMode::Native => None,
        }
    }

    fn enum_check_clause(&self, column: &Column) -> Option<String> {
        let enum_type = column.enum_type()?;
        let enum_values = self
            .context
            .settings()
            .database_model()
            .find_enum_type(column.schema_name(), enum_type)
            .values()
            .clone();

        let joined_values = enum_values
            .iter()
            .map(|value| format!("'{}'", escape_sql_literal(value.code())))
            .collect::<Vec<_>>()
            .join(",");

        Some(format!("{} in ({})", column.name(), joined_values))
    }

    fn min_max_check_clause(&self, column: &Column) -> String {
        let min_value = column.min_value();
        let max_value = column.max_value();
        let mut sql = String::new();

        if let Some(min_value) = min_value {
            sql.push_str(column.name());
            sql.push_str(" >= ");
            sql.push_str(min_value);
        }

        if min_value.is_some() && max_value.is_some() {
            sql.push_str(" and ");
        }

        if let Some(max_value) = max_value {
            sql.push_str(column.name());
            sql.push_str(" <= ");
            sql.push_str(max_value);
        }

        sql
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

/// Index of the `)` closing the `(` at index 0 of `text`, skipping quoted text.
fn closing_paren_index(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (index, ch) in text.char_indices() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '(' => depth += 1,
                ')' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            },
        }
    }
    None
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
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(varcharWithCheck = 'ABC123')"));
    }

    #[test]
    fn user_check_with_unrelated_leading_parens_is_not_unwrapped() {
        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "code", ColumnType::Varchar)
                    .length(10)
                    .check_constraint(Some("check (a > 1) or (b < 2)".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let constraints =
            DefaultColumnConstraintGenerator::new(ctx).column_check_constraints(&table);

        assert!(
            constraints[0].contains("check(check (a > 1) or (b < 2))"),
            "{constraints:?}"
        );
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
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("check(code <> '')"));
        assert!(!constraints[0].contains("check(check("));
    }

    #[test]
    fn user_check_and_min_max_are_combined_instead_of_min_max_being_dropped() {
        // Regression test for M3: a column with both a `<check>` and minValue/maxValue
        // used to silently lose the min/max half, since the old if/else-if chain only
        // ever rendered the first attribute it found.
        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "amount", ColumnType::Int)
                    .check_constraint(Some("amount <> 13".to_string()))
                    .min_value(Some("0".to_string()))
                    .max_value(Some("100".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(
            constraints[0].contains("check((amount <> 13) and (amount >= 0 and amount <= 100))"),
            "unexpected output: {}",
            constraints[0]
        );
    }

    #[test]
    fn already_wrapped_user_check_is_unwrapped_before_combining_with_min_max() {
        // A user-supplied check that already includes its own `check(...)` wrapper must
        // still combine cleanly with min/max, rather than nesting a stray inner `check(`.
        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "amount", ColumnType::Int)
                    .check_constraint(Some("check(amount <> 13)".to_string()))
                    .min_value(Some("0".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(
            constraints[0].contains("check((amount <> 13) and (amount >= 0))"),
            "unexpected output: {}",
            constraints[0]
        );
        assert!(!constraints[0].contains("check(check("));
    }

    #[test]
    fn boolean_column_with_min_max_combines_both_instead_of_dropping_min_max() {
        // Regression test for M3 (the follow-up H6 explicitly left open): a Boolean
        // column with min/max set under BooleanMode::Native used to drop the min/max
        // entirely, because the Boolean branch always matched first and returned `None`
        // under Native mode regardless of any other attributes also being set.
        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "flag", ColumnType::Boolean)
                    .min_value(Some("0".to_string()))
                    .max_value(Some("1".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(
            constraints[0].contains("check(flag >= 0 and flag <= 1)"),
            "unexpected output: {}",
            constraints[0]
        );
    }

    #[test]
    fn boolean_column_with_non_native_mode_and_min_max_combines_both() {
        // Under YesNo/YN mode the Boolean restriction itself is a real clause, so this
        // exercises the true multi-clause combination path (two clauses, not one
        // contributing None). `GenerateOptions::boolean_mode` (not `DatabaseModel`'s own
        // field) is what `SqlGeneratorSettings`/the generator actually reads, so this is
        // built directly rather than via `make_context`, matching
        // `test_support::make_context_with_fk_mode`'s pattern for the analogous override.
        use crate::common::generate_options::GenerateOptions;
        use crate::common::generator_context::GeneratorContext;
        use crate::common::print_writer::PrintWriter;
        use crate::common::sql_generator_settings::SqlGeneratorSettings;
        use crate::common::sql_writer::SqlWriter;
        use std::cell::RefCell;
        use std::rc::Rc;

        let table = TableBuilder::new(None::<&str>, "test")
            .add_column(
                ColumnBuilder::new(None::<&str>, "flag", ColumnType::Boolean)
                    .min_value(Some("0".to_string()))
                    .build(),
            )
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model =
            DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        let mut options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );
        options.boolean_mode = BooleanMode::YesNo;
        let settings = SqlGeneratorSettings::new(DatabaseType::Postgresql, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let ctx = GeneratorContext::new(settings, writer);

        let generator = DefaultColumnConstraintGenerator::new(ctx);
        let constraints = generator.column_check_constraints(&table);

        assert_eq!(constraints.len(), 1);
        assert!(
            constraints[0].contains("check((flag in ('Yes','No')) and (flag >= 0))"),
            "unexpected output: {}",
            constraints[0]
        );
    }
}
