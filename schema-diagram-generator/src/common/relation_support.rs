use schema_model::model::relation::Relation;
use schema_model::model::table::Table;

/// Uppercased display name for a table, schema-qualified when it lives in a named schema
/// so same-named tables in different schemas stay distinguishable in the diagram.
pub fn table_display_name(table: &Table) -> String {
    match table.schema_name() {
        Some(schema) => format!("{}.{}", schema, table.name()).to_uppercase(),
        None => table.name().to_uppercase(),
    }
}

/// Resolves a relation's `to_table_name` (`table` or `schema.table`; an unqualified name
/// means the default schema, as everywhere else in the model) to an index into `tables`.
pub fn resolve_relation_target(tables: &[&Table], reference: &str) -> Option<usize> {
    let (schema, name) = match reference.split('.').collect::<Vec<_>>().as_slice() {
        [name] => (None, *name),
        [schema, name] => (Some(*schema), *name),
        _ => return None,
    };
    tables.iter().position(|t| {
        t.name().eq_ignore_ascii_case(name)
            && match (t.schema_name(), schema) {
                (None, None) => true,
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => false,
            }
    })
}

/// Crow's-foot cardinality for a foreign key, derived from the nullability of its local
/// columns: the child side is always zero-or-many, and the parent side is exactly one
/// only when every FK column is required (otherwise the reference is optional).
pub fn relation_cardinality(table: &Table, relation: &Relation) -> &'static str {
    let all_required = relation
        .column_pairs()
        .iter()
        .all(|(from, _)| table.has_column(from) && table.column(from).is_required());
    if all_required { "}o--||" } else { "}o--o|" }
}
