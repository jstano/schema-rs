//! Shared constraint/index naming rules.
//!
//! This is the single source of truth for how primary key, unique key, index, and
//! foreign key identifiers are built. `schema-sql-generator` (the create path) and
//! `schema-migration-generator` (the diff-driven alter path) both call into this module
//! so the name a migration tries to `DROP`/reference is always exactly the name the
//! create path would have produced for the same object - see H20 in BUGS_AND_GAPS.

use crate::model::types::DatabaseType;

const PK_PREFIX: &str = "pk_";
const AK_PREFIX: &str = "ak_";
const IX_PREFIX: &str = "ix_";
const FK_PREFIX: &str = "fk_";

/// Builds a `<prefix><table><suffix>` identifier, lower-cased and truncated - by char,
/// never by byte index, so a multi-byte UTF-8 table name can't panic - so it fits the
/// target database's max identifier length.
fn build_name(prefix: &str, table_name: &str, suffix: &str, max_len: usize) -> String {
    let full_name = format!("{prefix}{table_name}{suffix}");
    if full_name.chars().count() <= max_len {
        return full_name.to_lowercase();
    }

    // Reserve space for the *actual* prefix/suffix length, not a hard-coded budget -
    // a table with >=10 siblings needs a 2-digit suffix.
    let available = max_len.saturating_sub(prefix.chars().count() + suffix.chars().count());
    let truncated_table_name: String = table_name.chars().take(available).collect();
    format!("{prefix}{truncated_table_name}{suffix}").to_lowercase()
}

/// Name of a table's primary key constraint, e.g. `pk_users`.
pub fn primary_key_name(database_type: DatabaseType, table_name: &str) -> String {
    build_name(PK_PREFIX, table_name, "", database_type.max_key_name_length())
}

/// Name of the `ordinal`-th (1-based) unique key constraint on a table, e.g. `ak_users1`.
/// `ordinal` counts only unique keys - it is the position of this key among the table's
/// `KeyType::Unique` entries, matching the create path's numbering.
pub fn unique_key_name(database_type: DatabaseType, table_name: &str, ordinal: usize) -> String {
    build_name(AK_PREFIX, table_name, &ordinal.to_string(), database_type.max_key_name_length())
}

/// Name of the `ordinal`-th (1-based) non-unique index on a table, e.g. `ix_users1`.
/// `ordinal` counts only indexes - the position of this key among the table's
/// `KeyType::Index` entries, matching the create path's numbering.
pub fn index_name(database_type: DatabaseType, table_name: &str, ordinal: usize) -> String {
    build_name(IX_PREFIX, table_name, &ordinal.to_string(), database_type.max_key_name_length())
}

/// Name of the `ordinal`-th (1-based) foreign key constraint on a table, e.g. `fk_orders1`.
/// `ordinal` is the position of this relation among all of the table's relations,
/// matching the create path's numbering.
pub fn foreign_key_name(database_type: DatabaseType, table_name: &str, ordinal: usize) -> String {
    build_name(FK_PREFIX, table_name, &ordinal.to_string(), database_type.max_key_name_length())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_key_name_has_no_ordinal() {
        assert_eq!(primary_key_name(DatabaseType::Postgresql, "users"), "pk_users");
    }

    #[test]
    fn unique_and_index_and_foreign_key_names_are_numbered() {
        assert_eq!(unique_key_name(DatabaseType::Postgresql, "users", 1), "ak_users1");
        assert_eq!(index_name(DatabaseType::Postgresql, "users", 2), "ix_users2");
        assert_eq!(foreign_key_name(DatabaseType::Postgresql, "orders", 1), "fk_orders1");
    }

    #[test]
    fn truncates_long_table_names_to_fit_max_key_name_length() {
        let long_table_name = "a".repeat(70);
        let name = foreign_key_name(DatabaseType::Postgresql, &long_table_name, 1);
        assert!(name.len() <= 63, "name '{}' exceeds postgres's 63 char limit", name);
        assert!(name.starts_with("fk_"));
        assert!(name.ends_with('1'));
    }

    #[test]
    fn truncates_multi_byte_table_names_without_panicking() {
        let long_table_name = "语".repeat(70);
        let name = index_name(DatabaseType::SqlServer, &long_table_name, 10);
        assert!(name.chars().count() <= 32);
        assert!(name.starts_with("ix_"));
        assert!(name.ends_with("10"));
    }
}
