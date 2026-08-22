use std::collections::{HashMap, HashSet};

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

/// Builds a collision-free mapping from each name in `names` to a safe diagram
/// identifier token. Sanitizing each name in isolation isn't enough: two *distinct*
/// names that happen to sanitize to the same token (e.g. `"Order-Detail"` and
/// `"Order Detail"` both -> `"Order_Detail"`) would otherwise silently collide and get
/// merged into a single diagram entity/attribute with no warning. This disambiguates by
/// appending a numeric suffix to every occurrence of a token after the first.
pub fn build_safe_identifier_map<'a, I>(names: I) -> HashMap<&'a str, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut used: HashSet<String> = HashSet::new();
    let mut map = HashMap::new();

    for name in names {
        let base = sanitize_token(name);
        let mut candidate = base.clone();
        let mut suffix = 2;
        while used.contains(&candidate) {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        used.insert(candidate.clone());
        map.insert(name, candidate);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_already_safe_names_unchanged() {
        let map = build_safe_identifier_map(["CUSTOMER", "ORDER_ITEM"]);
        assert_eq!(map["CUSTOMER"], "CUSTOMER");
        assert_eq!(map["ORDER_ITEM"], "ORDER_ITEM");
    }

    #[test]
    fn sanitizes_unsafe_characters() {
        let map = build_safe_identifier_map(["ORDER DETAIL"]);
        assert_eq!(map["ORDER DETAIL"], "ORDER_DETAIL");
    }

    #[test]
    fn disambiguates_distinct_names_that_sanitize_to_the_same_token() {
        let map = build_safe_identifier_map(["ORDER-DETAIL", "ORDER DETAIL", "ORDER_DETAIL"]);
        let ids: HashSet<&String> = map.values().collect();
        assert_eq!(ids.len(), 3, "all three distinct names must map to distinct ids: {map:?}");
        // The first occurrence keeps the unsuffixed token so the common (already-safe)
        // case is unaffected.
        assert_eq!(map["ORDER-DETAIL"], "ORDER_DETAIL");
    }
}
