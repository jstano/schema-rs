use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::BooleanMode;
use schema_sql_generator::common::constraint_naming::hashed_constraint_name;
use schema_sql_generator::common::sql_string::escape_sql_literal;

const CK_PREFIX: &str = "ck_";

/// The `ck_<table>_<column>_<hash>` name the full `CREATE TABLE` generator would give this
/// column's check constraint (see `DefaultColumnConstraintGenerator::constraint_name`) - kept
/// in sync so a migration-generated constraint matches what a fresh install would produce.
pub fn constraint_name(table_name: &str, column_name: &str) -> String {
    hashed_constraint_name(CK_PREFIX, table_name, column_name)
}

/// The bare boolean expression (no `check(...)` wrapper) for the CHECK constraint the full
/// `CREATE TABLE` generator would attach to this column - see
/// `DefaultColumnConstraintGenerator::check_constraint_sql`. Mirrors that logic (boolean
/// `YesNo`/`YN` modes, an explicit `check` attribute, enum value lists, and min/max bounds)
/// so `AddColumn` migrations enforce the same constraints a fresh install would.
pub fn check_expr(database_model: &DatabaseModel, column: &Column) -> Option<String> {
    if column.column_type() == ColumnType::Boolean {
        boolean_check_expr(database_model.boolean_mode(), column)
    } else if let Some(constraint) = column.check_constraint() {
        Some(strip_check_wrapper(constraint))
    } else if column.column_type() == ColumnType::Enum {
        enum_check_expr(database_model, column)
    } else if column.has_min_or_max_value() {
        min_max_check_expr(column)
    } else {
        None
    }
}

fn strip_check_wrapper(expression: &str) -> String {
    let trimmed = expression.trim();
    if trimmed.to_ascii_lowercase().starts_with("check")
        && let (Some(start), true) = (trimmed.find('('), trimmed.ends_with(')'))
    {
        return trimmed[start + 1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

fn boolean_check_expr(boolean_mode: BooleanMode, column: &Column) -> Option<String> {
    match boolean_mode {
        BooleanMode::YesNo => Some(format!("{} in ('Yes','No')", column.name())),
        BooleanMode::YN => Some(format!("{} in ('Y','N')", column.name())),
        BooleanMode::Native => None,
    }
}

fn enum_check_expr(database_model: &DatabaseModel, column: &Column) -> Option<String> {
    let enum_type_name = column.enum_type()?;
    let enum_type = database_model.find_enum_type(column.schema_name(), enum_type_name);

    let joined_values = enum_type
        .values()
        .iter()
        .map(|value| format!("'{}'", escape_sql_literal(value.code())))
        .collect::<Vec<_>>()
        .join(",");

    Some(format!("{} in ({})", column.name(), joined_values))
}

fn min_max_check_expr(column: &Column) -> Option<String> {
    let min_value = column.min_value();
    let max_value = column.max_value();
    let mut expr = String::new();

    if let Some(min_value) = min_value {
        expr.push_str(column.name());
        expr.push_str(" >= ");
        expr.push_str(min_value.to_string().as_str());
    }

    if min_value.is_some() && max_value.is_some() {
        expr.push_str(" and ");
    }

    if let Some(max_value) = max_value {
        expr.push_str(column.name());
        expr.push_str(" <= ");
        expr.push_str(max_value.to_string().as_str());
    }

    Some(expr)
}
