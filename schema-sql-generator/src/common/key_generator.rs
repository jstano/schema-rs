use schema_model::model::table::Table;
use crate::common::generator_context::GeneratorContext;

const PK_PREFIX: &str = "pk_";
const AK_PREFIX: &str = "ak_";

pub trait KeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String>;
}

pub struct DefaultKeyGenerator {
    context: GeneratorContext,
}

impl DefaultKeyGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }

    fn constraint_name(&self, prefix: &str, table_name: &str, suffix: Option<usize>) -> String {
        let database_type = self.context.settings().database_type();
        let max_key_name_length = database_type.max_key_name_length();

        let suffix_str = suffix.map(|n| n.to_string()).unwrap_or_default();
        let full_name = if suffix.is_some() {
            format!("{}{}{}", prefix, table_name, suffix_str)
        } else {
            format!("{}{}", prefix, table_name)
        };

        if full_name.len() <= max_key_name_length {
            return full_name.to_lowercase();
        }

        // Truncate table name to fit within max_key_name_length
        let suffix_len = suffix_str.len();
        let prefix_len = prefix.len();
        let available = max_key_name_length.saturating_sub(prefix_len + suffix_len);
        let truncated_table_name = &table_name[..available.min(table_name.len())];

        format!("{}{}{}", prefix, truncated_table_name, suffix_str).to_lowercase()
    }
}

impl KeyGenerator for DefaultKeyGenerator {
    fn key_constraints(&self, table: &Table) -> Vec<String> {
        let mut constraints = Vec::new();
        let mut unique_key_counter = 0;

        for key in table.keys() {
            if key.is_index() {
                continue;
            }

            match key.key_type() {
                schema_model::model::types::KeyType::Primary => {
                    let constraint_name = self.constraint_name(PK_PREFIX, table.name(), None);
                    constraints.push(format!(
                        "   constraint {} primary key ({})",
                        constraint_name,
                        key.columns_as_string()
                    ));
                }
                schema_model::model::types::KeyType::Unique => {
                    unique_key_counter += 1;
                    let constraint_name = self.constraint_name(AK_PREFIX, table.name(), Some(unique_key_counter));
                    constraints.push(format!(
                        "   constraint {} unique ({})",
                        constraint_name,
                        key.columns_as_string()
                    ));
                }
                schema_model::model::types::KeyType::Index => unreachable!(),
            }
        }

        constraints
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema_model::builder::TableBuilder;
    use schema_model::model::key::Key;
    use schema_model::model::key::KeyColumn;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, KeyType};
    use schema_model::model::database_model::DatabaseModel;
    use crate::common::test_support::make_context;

    #[test]
    fn empty_keys_returns_empty_vec() {
        let table = TableBuilder::new(None::<&str>, "users").build();
        let schema = schema_model::builder::SchemaBuilder::new(None::<&str>)
            .add_table(table.clone())
            .build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
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
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultKeyGenerator::new(ctx);
        let constraints = generator.key_constraints(&table);
        assert_eq!(constraints.len(), 2);

        // pk_* constraint name should be <= 63 chars
        let pk_constraint = &constraints[0];
        assert!(pk_constraint.contains("constraint pk_"));
        assert!(pk_constraint.contains("primary key (id)"));
        // Extract the constraint name part (between "constraint " and " primary")
        if let Some(start) = pk_constraint.find("constraint ") {
            if let Some(end) = pk_constraint.find(" primary") {
                let constraint_name = &pk_constraint[start + 11..end];
                assert!(constraint_name.len() <= 63, "Constraint name {} exceeds 63 chars", constraint_name);
            }
        }

        // ak_*1 constraint name should be <= 63 chars
        let ak_constraint = &constraints[1];
        assert!(ak_constraint.contains("constraint ak_"));
        assert!(ak_constraint.contains("unique (code)"));
        // Extract the constraint name part
        if let Some(start) = ak_constraint.find("constraint ") {
            if let Some(end) = ak_constraint.find(" unique") {
                let constraint_name = &ak_constraint[start + 11..end];
                assert!(constraint_name.len() <= 63, "Constraint name {} exceeds 63 chars", constraint_name);
            }
        }
    }
}
