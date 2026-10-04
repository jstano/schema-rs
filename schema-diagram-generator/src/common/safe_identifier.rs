use std::collections::HashSet;

/// Maps every character that isn't safe as a diagram identifier/attribute token
/// (alphanumeric or underscore) to `_` - both Mermaid's `erDiagram` and PlantUML's
/// entity syntax are effectively whitespace/punctuation-delimited, so a raw table or
/// column name containing e.g. a space or quote (plausible for a reverse-engineered
/// legacy schema with quoted identifiers) would otherwise corrupt the diagram's entity
/// or attribute boundaries.
pub fn sanitize_token(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// Builds a collision-free list of safe diagram identifier tokens, one per entry of
/// `names` and in the same order. Sanitizing each name in isolation isn't enough: two
/// *distinct* names that happen to sanitize to the same token (e.g. `"Order-Detail"` and
/// `"Order Detail"` both -> `"Order_Detail"`) would otherwise silently collide and get
/// merged into a single diagram entity/attribute with no warning. This disambiguates by
/// appending a numeric suffix to every occurrence of a token after the first.
///
/// The result is positional rather than a name-keyed map so that two entries with the
/// *same* name (e.g. `sales.users` and `auth.users` both named `users`) still get
/// distinct ids instead of the later one overwriting the earlier.
pub fn build_safe_identifiers<'a, I>(names: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut used: HashSet<String> = HashSet::new();
    let mut ids = Vec::new();

    for name in names {
        let base = sanitize_token(name);
        let mut candidate = base.clone();
        let mut suffix = 2;
        while used.contains(&candidate) {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        used.insert(candidate.clone());
        ids.push(candidate);
    }

    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_already_safe_names_unchanged() {
        assert_eq!(build_safe_identifiers(["CUSTOMER", "ORDER_ITEM"]), ["CUSTOMER", "ORDER_ITEM"]);
    }

    #[test]
    fn sanitizes_unsafe_characters() {
        assert_eq!(build_safe_identifiers(["ORDER DETAIL"]), ["ORDER_DETAIL"]);
    }

    #[test]
    fn disambiguates_distinct_names_that_sanitize_to_the_same_token() {
        let ids = build_safe_identifiers(["ORDER-DETAIL", "ORDER DETAIL", "ORDER_DETAIL"]);
        let unique: HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), 3, "all three distinct names must map to distinct ids: {ids:?}");
        // The first occurrence keeps the unsuffixed token so the common (already-safe)
        // case is unaffected.
        assert_eq!(ids[0], "ORDER_DETAIL");
    }

    #[test]
    fn identical_names_get_distinct_ids() {
        // M31: a name-keyed map overwrote the first entry with the second.
        let ids = build_safe_identifiers(["USERS", "USERS"]);
        assert_eq!(ids, ["USERS", "USERS_2"]);
    }
}
