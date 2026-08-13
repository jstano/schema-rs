/// Java-compatible `String.hashCode()`: a 32-bit, base-31 polynomial hash over UTF-16 code
/// units. All identifiers this is used for are ASCII, so iterating `char`s is equivalent.
pub fn java_string_hash(s: &str) -> u32 {
    s.chars()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32)) as u32
}

/// Lower-cases `s` and truncates it to at most `max_len` characters.
pub fn truncate_lower(s: &str, max_len: usize) -> String {
    s.to_lowercase().chars().take(max_len).collect()
}

/// Builds a `{prefix}{table[0..9]}_{column[0..9]}_{HASH}` constraint name, matching the
/// legacy Java codegen tool's naming scheme for hash-suffixed constraints (check constraints,
/// default constraints). The hash is computed over the full (non-truncated) lowercase
/// `"{table}_{column}"` string and formatted as uppercase hex, unpadded (matching Java's
/// `Integer.toHexString(hash).toUpperCase()` - not zero-padded to a fixed width).
pub fn hashed_constraint_name(prefix: &str, table_name: &str, column_name: &str) -> String {
    let table_lower = table_name.to_lowercase();
    let column_lower = column_name.to_lowercase();
    let hash = java_string_hash(&format!("{}_{}", table_lower, column_lower));
    let table_part = truncate_lower(&table_lower, 9);
    let column_part = truncate_lower(&column_lower, 9);

    format!("{}{}_{}_{:X}", prefix, table_part, column_part, hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_string_hash_matches_known_values() {
        // Verified by reproducing Java's String.hashCode() against every check/default
        // constraint name in the legacy tool's SQL Server reference output.
        assert_eq!(java_string_hash("earning_type_id"), 0x97E3620B);
        assert_eq!(java_string_hash("user_id"), 0xF73AEE0F);
        assert_eq!(
            java_string_hash("tip_pool_contributor_template_contribution_basis"),
            0x326125D1
        );
    }

    #[test]
    fn hashed_constraint_name_matches_legacy_format() {
        assert_eq!(
            hashed_constraint_name("df_", "earning_type", "id"),
            "df_earning_t_id_97E3620B"
        );
        assert_eq!(
            hashed_constraint_name("ck_", "tip_pool_contributor_template", "contribution_basis"),
            "ck_tip_pool__contribut_326125D1"
        );
    }

    #[test]
    fn hashed_constraint_name_does_not_zero_pad_the_hash() {
        // The legacy Java tool formats the hash with Integer.toHexString(...).toUpperCase(),
        // which drops leading zero nibbles rather than zero-padding to a fixed width.
        assert_eq!(java_string_hash("tip_sub_pool_template_hours_mode"), 0x0DCD6758);
        assert_eq!(
            hashed_constraint_name("ck_", "tip_sub_pool_template", "hours_mode"),
            "ck_tip_sub_p_hours_mod_DCD6758"
        );
    }
}
