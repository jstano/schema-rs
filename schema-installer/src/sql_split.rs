use schema_sql_generator::common::generator_type::GeneratorType;

/// Splits a SQL script into individual statements, respecting quoting and
/// comment rules so that semicolons (or, for SQL Server, `GO` batch
/// separators) embedded inside string/identifier/dollar-quoted literals or
/// comments are not treated as statement boundaries.
///
/// Returns trimmed, non-empty statements only.
pub(crate) fn split_sql_statements(sql: &str, database_type: &GeneratorType) -> Vec<String> {
    match database_type {
        GeneratorType::SqlServer => split_on_go_batches(sql),
        _ => split_on_semicolons(sql),
    }
}

enum State {
    Normal,
    SingleQuote,
    DoubleQuote,
    DollarQuote,
    LineComment,
    BlockComment,
}

/// Context-aware `;`-splitter for Postgres/SQLite dialects.
fn split_on_semicolons(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();

    let mut statements = Vec::new();
    let mut current_start = 0usize;
    let mut state = State::Normal;
    let mut dollar_tag = String::new();

    let mut i = 0usize;
    while i < len {
        let c = chars[i];

        match state {
            State::Normal => {
                if c == '\'' {
                    state = State::SingleQuote;
                    i += 1;
                } else if c == '"' {
                    state = State::DoubleQuote;
                    i += 1;
                } else if c == '-' && i + 1 < len && chars[i + 1] == '-' {
                    state = State::LineComment;
                    i += 2;
                } else if c == '/' && i + 1 < len && chars[i + 1] == '*' {
                    state = State::BlockComment;
                    i += 2;
                } else if c == '$' {
                    if let Some((tag, end)) = try_match_dollar_tag_open(&chars, i) {
                        dollar_tag = tag;
                        state = State::DollarQuote;
                        i = end;
                    } else {
                        i += 1;
                    }
                } else if c == ';' {
                    statements.push(chars[current_start..i].iter().collect::<String>());
                    current_start = i + 1;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            State::SingleQuote => {
                if c == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        i += 2;
                    } else {
                        state = State::Normal;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            State::DoubleQuote => {
                if c == '"' {
                    if i + 1 < len && chars[i + 1] == '"' {
                        i += 2;
                    } else {
                        state = State::Normal;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            State::DollarQuote => {
                if c == '$' {
                    if let Some(end) = try_match_dollar_tag_close(&chars, i, &dollar_tag) {
                        state = State::Normal;
                        dollar_tag.clear();
                        i = end;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            State::LineComment => {
                if c == '\n' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::BlockComment => {
                if c == '*' && i + 1 < len && chars[i + 1] == '/' {
                    state = State::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }

    if current_start < len {
        statements.push(chars[current_start..len].iter().collect::<String>());
    }

    statements
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// If `chars[pos]` starts a dollar-quote opening tag (`$`, then zero or more
/// identifier chars, then `$`), returns the tag text and the index just past
/// the opening `$...$`.
fn try_match_dollar_tag_open(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let mut j = pos + 1;
    let mut tag = String::new();
    while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
        tag.push(chars[j]);
        j += 1;
    }
    if j < chars.len() && chars[j] == '$' {
        Some((tag, j + 1))
    } else {
        None
    }
}

/// If `chars[pos]` (a `$`) begins the closing `$tag$` matching `tag`, returns
/// the index just past it. Only an exact tag match closes a dollar-quoted
/// string.
fn try_match_dollar_tag_close(chars: &[char], pos: usize, tag: &str) -> Option<usize> {
    let tag_chars: Vec<char> = tag.chars().collect();
    let end_of_tag = pos + 1 + tag_chars.len();
    if end_of_tag >= chars.len() {
        return None;
    }
    if chars[pos + 1..end_of_tag] == tag_chars[..] && chars[end_of_tag] == '$' {
        Some(end_of_tag + 1)
    } else {
        None
    }
}

/// SQL Server batch splitter: splits on lines that consist solely of the
/// token `GO` (case-insensitive), ignoring surrounding whitespace.
fn split_on_go_batches(sql: &str) -> Vec<String> {
    let mut batches = Vec::new();
    let mut current = String::new();

    for line in sql.lines() {
        if is_go_batch_separator(line) {
            batches.push(std::mem::take(&mut current));
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.is_empty() {
        batches.push(current);
    }

    batches
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// A line is a `GO` batch separator iff, once trimmed of surrounding
/// whitespace, it is exactly `GO` in any letter case.
fn is_go_batch_separator(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("go")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_postgres_statements() {
        let sql = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
        assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
    }

    #[test]
    fn splits_simple_sqlserver_go_batches() {
        let sql = "CREATE TABLE t1 (id INT)\nGO\nCREATE TABLE t2 (id INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
        assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
    }

    #[test]
    fn sqlserver_go_inside_identifier_is_not_a_separator() {
        let sql = "CREATE TABLE t1 (EGO INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("EGO"));
    }

    #[test]
    fn does_not_split_inside_single_quoted_string_containing_semicolon() {
        let sql = "INSERT INTO t1 (name) VALUES ('a;b'); INSERT INTO t1 (name) VALUES ('c');";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("'a;b'"));
    }

    #[test]
    fn handles_escaped_single_quotes() {
        let sql = "INSERT INTO t1 (name) VALUES ('it''s; fine'); SELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("it''s; fine"));
    }

    #[test]
    fn handles_double_quoted_identifiers_with_semicolon_and_escaped_quote() {
        let sql = r#"SELECT * FROM "weird;table" WHERE "a""b" = 1; SELECT 2;"#;
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains(r#""weird;table""#));
    }

    #[test]
    fn does_not_split_inside_line_comment() {
        let sql = "SELECT 1; -- comment with a ; semicolon\nSELECT 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn does_not_split_inside_block_comment() {
        let sql = "SELECT 1; /* comment ; with ; semicolons */ SELECT 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn does_not_split_inside_bare_dollar_quote() {
        let sql = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$\nBEGIN\n  PERFORM 1; PERFORM 2;\nEND;\n$$; SELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("PERFORM 1; PERFORM 2;"));
    }

    #[test]
    fn does_not_split_inside_tagged_dollar_quote() {
        let sql = "do $createextensions$\nbegin\n  create extension if not exists pgcrypto;\nend\n$createextensions$;\nSELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("create extension if not exists pgcrypto;"));
    }

    #[test]
    fn different_dollar_tags_do_not_close_each_other() {
        let sql = "SELECT $foo$ this has $$ inside it $foo$ as literal;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn dollar_sign_not_forming_a_quote_is_left_alone() {
        let sql = "INSERT INTO prices (amount) VALUES ('$5'); SELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("'$5'"));
    }

    #[test]
    fn full_v1_create_books_style_repro() {
        let sql = r#"
create table books (
    id uuid primary key,
    title varchar(200) not null
);

create or replace function generate_uuid() returns uuid language plpgsql parallel safe as $$
declare
   unix_time_ms CONSTANT bytea NOT NULL DEFAULT substring(int8send(floor(extract(epoch from clock_timestamp()) * 1000)::bigint) from 3);
   buffer bytea not null default unix_time_ms || gen_random_bytes(10);
begin
   buffer := set_byte(buffer, 6, (b'0111' || get_byte(buffer, 6)::bit(4))::bit(8)::int);
   buffer := set_byte(buffer, 8, (b'10' || get_byte(buffer, 8)::bit(6))::bit(8)::int);
   return encode(buffer, 'hex')::uuid;
end
$$;

do $createextensions$
begin
   create extension if not exists pgcrypto;
end
$createextensions$;

insert into books (id, title) values (generate_uuid(), 'Book; With Semicolon');
"#;
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 4);
        assert!(result[0].starts_with("create table books"));
        assert!(result[1].starts_with("create or replace function generate_uuid"));
        assert!(result[1].contains("$$"));
        assert!(result[1].trim_end().ends_with("$$"));
        assert!(result[2].starts_with("do $createextensions$"));
        assert!(result[2].trim_end().ends_with("$createextensions$"));
        assert!(result[3].contains("Book; With Semicolon"));
    }

    #[test]
    fn empty_and_whitespace_only_statements_are_filtered() {
        let sql = "  ;\nCREATE TABLE t1 (id INT);\n\n  ;  ";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
    }

    #[test]
    fn sqlite_uses_semicolon_splitting_like_postgres() {
        let sql = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
        let result = split_sql_statements(sql, &GeneratorType::Sqlite);
        assert_eq!(result.len(), 2);
    }
}
