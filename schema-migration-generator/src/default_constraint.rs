use schema_sql_generator::common::constraint_naming::hashed_constraint_name;

const DF_PREFIX: &str = "df_";

/// The `df_<table>_<column>_<hash>` name the full `CREATE TABLE` generator would give this
/// column's default constraint (see `DefaultConstraintNaming::NamedWithHash` in
/// `schema-sql-generator`'s `DefaultColumnGenerator`) - kept in sync so a migration-generated
/// constraint matches what a fresh install would produce, and so `DropColumn` can target it by
/// a computed name rather than inspecting the live database.
pub fn constraint_name(table_name: &str, column_name: &str) -> String {
    hashed_constraint_name(DF_PREFIX, table_name, column_name)
}
