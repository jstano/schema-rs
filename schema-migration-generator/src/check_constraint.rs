use schema_model::model::column::Column;
use schema_sql_generator::common::column_constraint_generator::DefaultColumnConstraintGenerator;
use schema_sql_generator::common::constraint_naming::hashed_constraint_name;
use schema_sql_generator::common::generator_context::GeneratorContext;

const CK_PREFIX: &str = "ck_";

/// The `ck_<table>_<column>_<hash>` name the full `CREATE TABLE` generator would give this
/// column's check constraint (see `DefaultColumnConstraintGenerator`'s private constraint
/// naming) - kept in sync so a migration-generated constraint matches what a fresh install
/// would produce.
pub fn constraint_name(table_name: &str, column_name: &str) -> String {
    hashed_constraint_name(CK_PREFIX, table_name, column_name)
}

/// The full `check(...)` clause the real `CREATE TABLE` generator would attach to this
/// column, delegated straight to `DefaultColumnConstraintGenerator::check_constraint_sql`
/// (boolean `YesNo`/`YN` modes, an explicit `check` attribute, enum value lists, and min/max
/// bounds) so `AddColumn` migrations enforce exactly the same constraints a fresh install
/// would, with no separate reimplementation to drift out of sync.
pub fn check_constraint_sql(context: &GeneratorContext, column: &Column) -> Option<String> {
    DefaultColumnConstraintGenerator::new(context.clone()).check_constraint_sql(column)
}
