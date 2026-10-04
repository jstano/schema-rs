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
///
/// PostgreSQL's limit (63) is in *bytes*; SQL Server's and SQLite's are in characters. The
/// budget is measured accordingly, otherwise a multi-byte table name would pass the char
/// check, then be silently truncated by PostgreSQL itself - potentially colliding with a
/// sibling whose only difference was the (cut-off) numeric suffix (M8). Lower-casing can
/// change a character's width, so the measurement is taken on the lower-cased result.
fn build_name(prefix: &str, table_name: &str, suffix: &str, database_type: DatabaseType) -> String {
    let max_len = database_type.max_key_name_length();
    let measure = |s: &str| {
        if database_type == DatabaseType::Postgresql {
            s.len()
        } else {
            s.chars().count()
        }
    };

    let full_name = format!("{prefix}{table_name}{suffix}").to_lowercase();
    if measure(&full_name) <= max_len {
        return full_name;
    }

    // Keep the suffix intact (it carries the ordinal that keeps sibling names distinct) and
    // drop table-name chars one at a time until the whole identifier fits.
    let prefix = prefix.to_lowercase();
    let suffix = suffix.to_lowercase();
    let mut table_name: String = table_name.to_lowercase();
    while !table_name.is_empty() && measure(&format!("{prefix}{table_name}{suffix}")) > max_len {
        table_name.pop();
    }
    format!("{prefix}{table_name}{suffix}")
}

/// Name of a table's primary key constraint, e.g. `pk_users`.
pub fn primary_key_name(database_type: DatabaseType, table_name: &str) -> String {
    build_name(PK_PREFIX, table_name, "", database_type)
}

/// Name of the `ordinal`-th (1-based) unique key constraint on a table, e.g. `ak_users1`.
/// `ordinal` counts only unique keys - it is the position of this key among the table's
/// `KeyType::Unique` entries, matching the create path's numbering.
pub fn unique_key_name(database_type: DatabaseType, table_name: &str, ordinal: usize) -> String {
    build_name(AK_PREFIX, table_name, &ordinal.to_string(), database_type)
}

/// Name of the `ordinal`-th (1-based) non-unique index on a table, e.g. `ix_users1`.
/// `ordinal` counts only indexes - the position of this key among the table's
/// `KeyType::Index` entries, matching the create path's numbering.
pub fn index_name(database_type: DatabaseType, table_name: &str, ordinal: usize) -> String {
    build_name(IX_PREFIX, table_name, &ordinal.to_string(), database_type)
}

/// Name of the `ordinal`-th (1-based) foreign key constraint on a table, e.g. `fk_orders1`.
/// `ordinal` is the position of this relation among all of the table's relations,
/// matching the create path's numbering.
pub fn foreign_key_name(database_type: DatabaseType, table_name: &str, ordinal: usize) -> String {
    build_name(FK_PREFIX, table_name, &ordinal.to_string(), database_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_key_name_has_no_ordinal() {
        assert_eq!(
            primary_key_name(DatabaseType::Postgresql, "users"),
            "pk_users"
        );
    }

    #[test]
    fn unique_and_index_and_foreign_key_names_are_numbered() {
        assert_eq!(
            unique_key_name(DatabaseType::Postgresql, "users", 1),
            "ak_users1"
        );
        assert_eq!(
            index_name(DatabaseType::Postgresql, "users", 2),
            "ix_users2"
        );
        assert_eq!(
            foreign_key_name(DatabaseType::Postgresql, "orders", 1),
            "fk_orders1"
        );
    }

    #[test]
    fn truncates_long_table_names_to_fit_max_key_name_length() {
        let long_table_name = "a".repeat(70);
        let name = foreign_key_name(DatabaseType::Postgresql, &long_table_name, 1);
        assert!(
            name.len() <= 63,
            "name '{}' exceeds postgres's 63 char limit",
            name
        );
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

    #[test]
    fn postgres_limit_is_measured_in_bytes_not_chars() {
        // 70 Cyrillic chars = 140 bytes; the old char-based budget kept 60 chars (120 bytes)
        // and left PostgreSQL to truncate further on its own.
        let long_table_name = "ж".repeat(70);
        for ordinal in [1, 2, 10] {
            let name = unique_key_name(DatabaseType::Postgresql, &long_table_name, ordinal);
            assert!(name.len() <= 63, "name '{}' is {} bytes", name, name.len());
            assert!(name.starts_with("ak_"));
            assert!(name.ends_with(&ordinal.to_string()));
        }
        assert_ne!(
            unique_key_name(DatabaseType::Postgresql, &long_table_name, 1),
            unique_key_name(DatabaseType::Postgresql, &long_table_name, 2)
        );
    }

    #[test]
    fn non_postgres_limits_remain_char_based() {
        let long_table_name = "ж".repeat(200);
        let name = primary_key_name(DatabaseType::Sqlite, &long_table_name);
        assert_eq!(name.chars().count(), 128);
    }
}
