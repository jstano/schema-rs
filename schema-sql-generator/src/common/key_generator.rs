use crate::common::generator_context::GeneratorContext;
use schema_model::model::key::Key;
use schema_model::model::table::Table;
use schema_model::model::types::DatabaseType;
use schema_model::naming::{primary_key_name, unique_key_name};

pub trait KeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultKeyGenerator {
    context: GeneratorContext,
    nonclustered_primary_key: bool,
}

impl DefaultKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
            nonclustered_primary_key: false,
        }
    }

    /// When set, primary key constraints render `primary key nonclustered (...)` instead of
    /// `primary key (...)` - matches SQL Server's legacy codegen convention.
    pub fn with_nonclustered_primary_key(mut self, value: bool) -> Self {
        self.nonclustered_primary_key = value;
        self
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }

    /// `cluster="true"` only has a rendering on SQL Server (`clustered`/`nonclustered` is a
    /// SQL Server-only constraint keyword - PostgreSQL and SQLite have nothing equivalent).
    /// On those dialects the attribute is ignored rather than hard-erroring, since one
    /// schema.xml is meant to target all three databases and a SQL-Server-only tuning hint
    /// shouldn't stop the other two from generating (M1).
    fn effective_cluster(&self, table_name: &str, key_kind: &str, key: &Key) -> bool {
        if !key.is_cluster() {
            return false;
        }

        let database_type = self.context.settings().database_type();
        if database_type == DatabaseType::SqlServer {
            true
        } else {
            eprintln!(
                "warning: table '{}' has a clustered {} constraint, but {:?} does not support clustered/nonclustered constraints -- ignoring",
                table_name, key_kind, database_type
            );
            false
        }
    }
}

impl DefaultKeyGenerator {
    /// Same as `key_constraints`, but when `skip_primary_key` is true the primary key
    /// constraint is left out entirely - for dialects (SQLite) that declare the primary key
    /// inline on the column instead of as a separate table constraint.
    pub fn key_constraints_filtered(&self, table: &Table, skip_primary_key: bool) -> Vec<String> {
        let mut constraints = Vec::new();
        let mut unique_key_counter = 0;

        for key in table.keys() {
            if key.is_index() {
                continue;
            }

            match key.key_type() {
                schema_model::model::types::KeyType::Primary => {
                    if skip_primary_key {
                        continue;
                    }

                    let database_type = self.context.settings().database_type();
                    let constraint_name = primary_key_name(database_type, table.name());
                    // `cluster="true"` always wins over the dialect's own nonclustered-by-
                    // default convention below - otherwise a SQL Server primary key
                    // explicitly asking to be clustered would render the opposite (M1).
                    let primary_key_clause = if self.effective_cluster(table.name(), "primary key", key) {
                        "primary key clustered"
                    } else if self.nonclustered_primary_key {
                        "primary key nonclustered"
                    } else {
                        "primary key"
                    };
                    constraints.push(format!(
                        "   constraint {} {} ({})",
                        constraint_name,
                        primary_key_clause,
                        key.columns_as_string()
                    ));
                }
                schema_model::model::types::KeyType::Unique => {
                    unique_key_counter += 1;
                    let database_type = self.context.settings().database_type();
                    let constraint_name = unique_key_name(database_type, table.name(), unique_key_counter);
                    let unique_clause = if self.effective_cluster(table.name(), "unique", key) {
                        "unique clustered"
                    } else {
                        "unique"
                    };
                    constraints.push(format!(
                        "   constraint {} {} ({})",
                        constraint_name,
                        unique_clause,
                        key.columns_as_string()
                    ));
                }
                schema_model::model::types::KeyType::Index => unreachable!(),
            }
        }

        constraints
    }
}

impl KeyGenerator for DefaultKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        self.key_constraints_filtered(table, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::TableBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::key::Key;
    use schema_model::model::key::KeyColumn;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, KeyType};

    #[test]
    fn empty_keys_returns_empty_vec() {
        let table = TableBuilder::new(None::<&str>, "users").build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        assert_eq!(generator.key_constraints(&table), vec![] as Vec<String>);
    }

    #[test]
    fn primary_key_single_column() {
        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        let table = TableBuilder::new(None::<&str>, "users")
            .add_key(pk)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0], "   constraint pk_users primary key (id)");
    }

    #[test]
    fn primary_key_multiple_columns() {
        let pk = Key::new(KeyType::Primary, vec![
            KeyColumn::new("org_id"),
            KeyColumn::new("user_id"),
        ]);
        let table = TableBuilder::new(None::<&str>, "users")
            .add_key(pk)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0], "   constraint pk_users primary key (org_id,user_id)");
    }

    #[test]
    fn unique_key_single_column() {
        let uq = Key::new(KeyType::Unique, vec![KeyColumn::new("email")]);
        let table = TableBuilder::new(None::<&str>, "users")
            .add_key(uq)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0], "   constraint ak_users1 unique (email)");
    }

    #[test]
    fn unique_key_multiple_columns() {
        let uq = Key::new(KeyType::Unique, vec![
            KeyColumn::new("tenant_id"),
            KeyColumn::new("code"),
        ]);
        let table = TableBuilder::new(None::<&str>, "products")
            .add_key(uq)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0], "   constraint ak_products1 unique (tenant_id,code)");
    }

    #[test]
    fn multiple_keys_primary_and_unique() {
        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        let uq1 = Key::new(KeyType::Unique, vec![KeyColumn::new("email")]);
        let uq2 = Key::new(KeyType::Unique, vec![KeyColumn::new("username")]);
        let table = TableBuilder::new(None::<&str>, "users")
            .add_key(pk)
            .add_key(uq1)
            .add_key(uq2)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 3);
        assert_eq!(constraints[0], "   constraint pk_users primary key (id)");
        assert_eq!(constraints[1], "   constraint ak_users1 unique (email)");
        assert_eq!(constraints[2], "   constraint ak_users2 unique (username)");
    }

    #[test]
    fn filters_out_index_keys() {
        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        let idx = Key::new(KeyType::Index, vec![KeyColumn::new("name")]);
        let table = TableBuilder::new(None::<&str>, "users")
            .add_key(pk)
            .add_index(idx)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        // Only primary key should be emitted, index should be filtered
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0], "   constraint pk_users primary key (id)");
    }

    #[test]
    fn unique_key_numbering_independent_of_primary_key() {
        let uq1 = Key::new(KeyType::Unique, vec![KeyColumn::new("email")]);
        let uq2 = Key::new(KeyType::Unique, vec![KeyColumn::new("username")]);
        let table = TableBuilder::new(None::<&str>, "products")
            .add_key(uq1)
            .add_key(uq2)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 2);
        assert_eq!(constraints[0], "   constraint ak_products1 unique (email)");
        assert_eq!(constraints[1], "   constraint ak_products2 unique (username)");
    }

    #[test]
    fn constraint_name_truncates_for_long_table_names() {
        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        let uq = Key::new(KeyType::Unique, vec![KeyColumn::new("code")]);
        let long_table_name = "a".repeat(70); // Exceeds postgres max of 63
        let table = TableBuilder::new(None::<&str>, &long_table_name)
            .add_key(pk)
            .add_key(uq)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 2);

        // pk_* constraint name should be <= 63 chars
        let pk_constraint = &constraints[0];
        assert!(pk_constraint.contains("constraint pk_"));
        assert!(pk_constraint.contains("primary key (id)"));
        // Extract the constraint name part (between "constraint " and " primary")
        if let Some(start) = pk_constraint.find("constraint ")
            && let Some(end) = pk_constraint.find(" primary") {
                let constraint_name = &pk_constraint[start + 11..end];
                assert!(constraint_name.len() <= 63, "Constraint name {} exceeds 63 chars", constraint_name);
            }

        // ak_*1 constraint name should be <= 63 chars
        let ak_constraint = &constraints[1];
        assert!(ak_constraint.contains("constraint ak_"));
        assert!(ak_constraint.contains("unique (code)"));
        // Extract the constraint name part
        if let Some(start) = ak_constraint.find("constraint ")
            && let Some(end) = ak_constraint.find(" unique") {
                let constraint_name = &ak_constraint[start + 11..end];
                assert!(constraint_name.len() <= 63, "Constraint name {} exceeds 63 chars", constraint_name);
            }
    }

    #[test]
    fn constraint_name_truncates_multi_byte_table_name_without_panicking() {
        // Regression test: byte-index slicing panics ("not a char boundary") on
        // multi-byte UTF-8 once truncation kicks in; truncating by char must not.
        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        let long_table_name = "语".repeat(70);
        let table = TableBuilder::new(None::<&str>, &long_table_name)
            .add_key(pk)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 1);
        assert!(constraints[0].contains("constraint pk_"));
    }

    #[test]
    fn with_nonclustered_primary_key_adds_nonclustered_keyword() {
        let pk = Key::new(KeyType::Primary, vec![KeyColumn::new("id")]);
        let uq = Key::new(KeyType::Unique, vec![KeyColumn::new("email")]);
        let table = TableBuilder::new(None::<&str>, "users")
            .add_key(pk)
            .add_key(uq)
            .build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultKeyGenerator::new(ctx).with_nonclustered_primary_key(true);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 2);
        assert_eq!(constraints[0], "   constraint pk_users primary key nonclustered (id)");
        // Unique keys are unaffected.
        assert_eq!(constraints[1], "   constraint ak_users1 unique (email)");
    }

    #[test]
    fn cluster_true_on_sql_server_overrides_nonclustered_primary_key_default() {
        // Regression test for M1: `sqlserver_key_generator.rs` hard-codes
        // `with_nonclustered_primary_key(true)`, so `<primary cluster="true">` used to
        // render `primary key nonclustered` - the exact opposite of what was asked.
        let pk = Key::new_full(KeyType::Primary, vec![KeyColumn::new("id")], true, false, false, None::<String>);
        let table = TableBuilder::new(None::<&str>, "users").add_key(pk).build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultKeyGenerator::new(ctx).with_nonclustered_primary_key(true);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints, vec!["   constraint pk_users primary key clustered (id)"]);
    }

    #[test]
    fn cluster_true_on_sql_server_unique_key_adds_clustered_keyword() {
        let uq = Key::new_full(KeyType::Unique, vec![KeyColumn::new("email")], true, false, false, None::<String>);
        let table = TableBuilder::new(None::<&str>, "users").add_key(uq).build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::SqlServer);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints, vec!["   constraint ak_users1 unique clustered (email)"]);
    }

    #[test]
    fn cluster_true_is_ignored_on_dialects_without_clustered_constraints() {
        // PostgreSQL/SQLite have no clustered/nonclustered constraint syntax - `cluster`
        // is ignored (with a warning) rather than rendering invalid SQL (M1).
        let pk = Key::new_full(KeyType::Primary, vec![KeyColumn::new("id")], true, false, false, None::<String>);
        let uq = Key::new_full(KeyType::Unique, vec![KeyColumn::new("email")], true, false, false, None::<String>);
        let table = TableBuilder::new(None::<&str>, "users").add_key(pk).add_key(uq).build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints[0], "   constraint pk_users primary key (id)");
        assert_eq!(constraints[1], "   constraint ak_users1 unique (email)");
    }
}
