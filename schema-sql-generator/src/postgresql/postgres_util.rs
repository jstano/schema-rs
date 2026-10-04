pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_underscore = false;

    for ch in s.chars() {
        if ch.is_uppercase() && !prev_underscore && !result.is_empty() {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
        prev_underscore = ch == '_';
    }

    result
}

/// The PostgreSQL name of an enum type: `schema.snake_case_name` for an enum owned by a
/// named schema, bare `snake_case_name` for the default schema. Qualifying matters because
/// an unqualified `create type` lands wherever `search_path` points, and two schemas
/// declaring a same-named enum would otherwise collide (M4).
pub fn qualified_enum_type_name(owner_schema: Option<&str>, enum_name: &str) -> String {
    match owner_schema {
        Some(schema) => format!("{}.{}", schema, to_snake_case(enum_name)),
        None => to_snake_case(enum_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("GenderType"), "gender_type");
        assert_eq!(to_snake_case("ShowInModuleType"), "show_in_module_type");
        assert_eq!(to_snake_case("TestEnumType"), "test_enum_type");
        assert_eq!(to_snake_case("ABC"), "a_b_c");
        assert_eq!(to_snake_case("simpleType"), "simple_type");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }
}
