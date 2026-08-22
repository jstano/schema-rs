/// Escapes `value` for safe interpolation inside a single-quoted SQL string literal, by
/// doubling any embedded single quote (the standard SQL escaping - understood by
/// Postgres, SQL Server, and SQLite alike). Every place in this crate that writes a
/// user-supplied value (identifier, enum code, configured username, ...) inside `'...'`
/// must go through this first: an unescaped embedded quote breaks the generated SQL at
/// best, or lets the value's content escape the string literal and inject arbitrary SQL
/// at worst.
pub fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_plain_text_unchanged() {
        assert_eq!(escape_sql_literal("hello"), "hello");
    }

    #[test]
    fn doubles_embedded_single_quotes() {
        assert_eq!(escape_sql_literal("O'Brien"), "O''Brien");
        assert_eq!(escape_sql_literal("it's a 'test'"), "it''s a ''test''");
    }
}
