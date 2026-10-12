use super::{SqlServerClient, query, string, strip_outer_parens};
use crate::error::SchemaReverseEngineerError;
use schema_model::model::constraint::Constraint;
use schema_model::model::types::DatabaseType;
use std::collections::HashMap;

/// Check constraints, grouped by owning table. Primary and unique keys are represented as table
/// keys instead; column defaults live on the column.
pub async fn list_constraints(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<HashMap<String, Vec<Constraint>>, SchemaReverseEngineerError> {
    let rows = query(
        client,
        "SELECT t.name AS table_name, cc.name AS name, cc.definition AS definition \
         FROM sys.check_constraints cc \
         JOIN sys.tables t ON t.object_id = cc.parent_object_id \
         WHERE t.schema_id = SCHEMA_ID(@P1) AND t.is_ms_shipped = 0 \
         ORDER BY t.name, cc.name",
        db_schema,
    )
    .await?;

    let mut by_table: HashMap<String, Vec<Constraint>> = HashMap::new();
    for row in &rows {
        by_table
            .entry(string(row, "table_name")?)
            .or_default()
            .push(Constraint::new(
                string(row, "name")?,
                check_sql(&string(row, "definition")?),
                DatabaseType::SqlServer,
            ));
    }
    Ok(by_table)
}

/// `sys.check_constraints.definition` is just the parenthesised predicate; the generator emits
/// `constraint <name> <sql>`, so the `check` keyword has to be part of the stored SQL.
fn check_sql(definition: &str) -> String {
    format!("check ({})", strip_outer_parens(definition))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_is_rewrapped_as_a_check_clause() {
        assert_eq!(check_sql("([amount]>=(0))"), "check ([amount]>=(0))");
        assert_eq!(check_sql("(([a]>(0)))"), "check ([a]>(0))");
    }
}
