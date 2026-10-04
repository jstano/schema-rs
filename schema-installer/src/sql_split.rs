use crate::error::SchemaInstallerError;
use schema_sql_generator::common::generator_type::GeneratorType;

/// Splits a SQL script into individual statements, respecting quoting and
/// comment rules so that semicolons (or, for SQL Server, `GO` batch
/// separators) embedded inside string/identifier/dollar-quoted literals or
/// comments are not treated as statement boundaries.
///
/// Returns trimmed, non-empty statements only, or an error if the script ends
/// inside an unterminated string, quoted identifier, dollar-quote or block comment
/// (M17) - splitting it anyway would yield one giant bogus "statement".
///
/// Dialect rules: PostgreSQL and SQL Server nest `/* */` comments, SQLite does not (M15);
/// only PostgreSQL has `E'...'` strings where `\` escapes a quote (M16).
pub(crate) fn split_sql_statements(
    sql: &str,
    database_type: &GeneratorType,
) -> Result<Vec<String>, SchemaInstallerError> {
    let rules = match database_type {
        GeneratorType::Postgresql => Rules {
            nested_comments: true,
            backslash_escapes: true,
        },
        GeneratorType::SqlServer => Rules {
            nested_comments: true,
            backslash_escapes: false,
        },
        _ => Rules {
            nested_comments: false,
            backslash_escapes: false,
        },
    };
    match database_type {
        GeneratorType::SqlServer => split_on_go_batches(sql, rules),
        _ => split_on_semicolons(sql, rules),
    }
}

#[derive(Clone, Copy)]
struct Rules {
    nested_comments: bool,
    backslash_escapes: bool,
}

/// True if the single quote at `pos` opens a PostgreSQL escape string (`E'...'`):
/// preceded by `e`/`E` that is not itself the tail of a longer identifier or number.
fn opens_escape_string(chars: &[char], pos: usize) -> bool {
    pos >= 1
        && matches!(chars[pos - 1], 'e' | 'E')
        && (pos < 2
            || !(chars[pos - 2].is_alphanumeric()
                || chars[pos - 2] == '_'
                || chars[pos - 2] == '$'))
}

fn unterminated(what: &str) -> SchemaInstallerError {
    SchemaInstallerError::Execution(format!(
        "unterminated {what} at end of SQL script - check for a missing closing delimiter"
    ))
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
fn split_on_semicolons(sql: &str, rules: Rules) -> Result<Vec<String>, SchemaInstallerError> {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();

    let mut statements = Vec::new();
    let mut current_start = 0usize;
    let mut state = State::Normal;
    let mut dollar_tag = String::new();
    let mut comment_depth = 0usize;
    let mut escape_string = false;

    let mut i = 0usize;
    while i < len {
        let c = chars[i];

        match state {
            State::Normal => {
                if c == '\'' {
                    escape_string = rules.backslash_escapes && opens_escape_string(&chars, i);
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
                    comment_depth = 1;
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
                if escape_string && c == '\\' && i + 1 < len {
                    i += 2;
                } else if c == '\'' {
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
                if rules.nested_comments && c == '/' && i + 1 < len && chars[i + 1] == '*' {
                    comment_depth += 1;
                    i += 2;
                } else if c == '*' && i + 1 < len && chars[i + 1] == '/' {
                    comment_depth -= 1;
                    if comment_depth == 0 {
                        state = State::Normal;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }

    match state {
        State::SingleQuote => return Err(unterminated("string literal")),
        State::DoubleQuote => return Err(unterminated("quoted identifier")),
        State::DollarQuote => return Err(unterminated("dollar-quoted string")),
        State::BlockComment => return Err(unterminated("block comment")),
        State::Normal | State::LineComment => {}
    }

    if current_start < len {
        statements.push(chars[current_start..len].iter().collect::<String>());
    }

    Ok(statements
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// If `chars[pos]` starts a dollar-quote opening tag (`$`, then zero or more
/// identifier chars, then `$`), returns the tag text and the index just past
/// the opening `$...$`.
fn try_match_dollar_tag_open(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let mut j = pos + 1;
    let mut tag = String::new();
    // PostgreSQL tags follow identifier rules: the first character can't be a digit, so
    // `$1$` is a positional parameter followed by a `$`, not a dollar-quote opening.
    if j < chars.len() && chars[j].is_ascii_digit() {
        return None;
    }
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

enum GoState {
    Normal,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

/// SQL Server batch splitter: splits on lines that consist solely of the token `GO`
/// (case-insensitive), ignoring surrounding whitespace - but, like `split_on_semicolons`,
/// tracks quote/comment state across the whole script first, so a `GO`-only line that's
/// actually inside a multi-line string literal or block comment is never mistaken for a
/// real batch separator.
fn split_on_go_batches(sql: &str, rules: Rules) -> Result<Vec<String>, SchemaInstallerError> {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();

    let mut batches = Vec::new();
    let mut current = String::new();
    let mut current_line = String::new();
    let mut state = GoState::Normal;
    let mut comment_depth = 0usize;
    // Whether the parser was in `Normal` state at the *start* of the line now being
    // scanned - only a line that starts and ends in `Normal` state can possibly be a
    // real top-level `GO` separator.
    let mut line_start_in_normal = true;

    let finish_line = |batches: &mut Vec<String>,
                       current: &mut String,
                       current_line: &mut String,
                       line_start_in_normal: bool| {
        if line_start_in_normal && let Some(count) = go_batch_separator_count(current_line) {
            // `GO n` runs the preceding batch n times.
            let batch = std::mem::take(current);
            for _ in 0..count {
                batches.push(batch.clone());
            }
        } else {
            current.push_str(current_line);
            current.push('\n');
        }
        current_line.clear();
    };

    let mut i = 0usize;
    while i < len {
        let c = chars[i];

        match state {
            GoState::Normal => {
                if c == '\'' {
                    state = GoState::SingleQuote;
                    current_line.push(c);
                    i += 1;
                } else if c == '"' {
                    state = GoState::DoubleQuote;
                    current_line.push(c);
                    i += 1;
                } else if c == '-' && i + 1 < len && chars[i + 1] == '-' {
                    state = GoState::LineComment;
                    current_line.push_str("--");
                    i += 2;
                } else if c == '/' && i + 1 < len && chars[i + 1] == '*' {
                    state = GoState::BlockComment;
                    comment_depth = 1;
                    current_line.push_str("/*");
                    i += 2;
                } else if c == '\n' {
                    finish_line(
                        &mut batches,
                        &mut current,
                        &mut current_line,
                        line_start_in_normal,
                    );
                    line_start_in_normal = true;
                    i += 1;
                } else {
                    current_line.push(c);
                    i += 1;
                }
            }
            GoState::SingleQuote => {
                if c == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        current_line.push_str("''");
                        i += 2;
                    } else {
                        current_line.push(c);
                        state = GoState::Normal;
                        i += 1;
                    }
                } else if c == '\n' {
                    finish_line(
                        &mut batches,
                        &mut current,
                        &mut current_line,
                        line_start_in_normal,
                    );
                    line_start_in_normal = false; // still inside the string on the next line
                    i += 1;
                } else {
                    current_line.push(c);
                    i += 1;
                }
            }
            GoState::DoubleQuote => {
                if c == '"' {
                    if i + 1 < len && chars[i + 1] == '"' {
                        current_line.push_str("\"\"");
                        i += 2;
                    } else {
                        current_line.push(c);
                        state = GoState::Normal;
                        i += 1;
                    }
                } else if c == '\n' {
                    finish_line(
                        &mut batches,
                        &mut current,
                        &mut current_line,
                        line_start_in_normal,
                    );
                    line_start_in_normal = false;
                    i += 1;
                } else {
                    current_line.push(c);
                    i += 1;
                }
            }
            GoState::LineComment => {
                if c == '\n' {
                    state = GoState::Normal;
                    finish_line(
                        &mut batches,
                        &mut current,
                        &mut current_line,
                        line_start_in_normal,
                    );
                    line_start_in_normal = true; // a line comment always ends at the newline
                    i += 1;
                } else {
                    current_line.push(c);
                    i += 1;
                }
            }
            GoState::BlockComment => {
                if rules.nested_comments && c == '/' && i + 1 < len && chars[i + 1] == '*' {
                    current_line.push_str("/*");
                    comment_depth += 1;
                    i += 2;
                } else if c == '*' && i + 1 < len && chars[i + 1] == '/' {
                    current_line.push_str("*/");
                    comment_depth -= 1;
                    if comment_depth == 0 {
                        state = GoState::Normal;
                    }
                    i += 2;
                } else if c == '\n' {
                    finish_line(
                        &mut batches,
                        &mut current,
                        &mut current_line,
                        line_start_in_normal,
                    );
                    line_start_in_normal = false; // still inside the comment on the next line
                    i += 1;
                } else {
                    current_line.push(c);
                    i += 1;
                }
            }
        }
    }
    match state {
        GoState::SingleQuote => return Err(unterminated("string literal")),
        GoState::DoubleQuote => return Err(unterminated("quoted identifier")),
        GoState::BlockComment => return Err(unterminated("block comment")),
        GoState::Normal | GoState::LineComment => {}
    }
    finish_line(
        &mut batches,
        &mut current,
        &mut current_line,
        line_start_in_normal,
    );
    if !current.is_empty() {
        batches.push(current);
    }

    Ok(batches
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// If `line` is a `GO` batch separator, returns how many times the preceding batch runs.
/// A separator is `GO`, in any letter case, optionally followed by a repeat count (`GO 5`)
/// and/or a trailing `--` line comment, with surrounding whitespace ignored. Anything else
/// after `GO` (e.g. `GO foo`, `GO 0`) means the line is not a separator.
fn go_batch_separator_count(line: &str) -> Option<usize> {
    let line = line.split("--").next().unwrap_or("").trim();
    let rest = line
        .get(..2)
        .filter(|head| head.eq_ignore_ascii_case("go"))?;
    let after_go = &line[rest.len()..];
    let rest = after_go.trim();
    if rest.is_empty() {
        return Some(1);
    }
    if !after_go.starts_with(char::is_whitespace) {
        return None;
    }
    match rest.parse::<usize>() {
        Ok(n) if n > 0 => Some(n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_postgres_statements() {
        let sql = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
        assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
    }

    #[test]
    fn splits_simple_sqlserver_go_batches() {
        let sql = "CREATE TABLE t1 (id INT)\nGO\nCREATE TABLE t2 (id INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
        assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
    }

    #[test]
    fn sqlserver_go_inside_identifier_is_not_a_separator() {
        let sql = "CREATE TABLE t1 (EGO INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("EGO"));
    }

    #[test]
    fn does_not_split_inside_single_quoted_string_containing_semicolon() {
        let sql = "INSERT INTO t1 (name) VALUES ('a;b'); INSERT INTO t1 (name) VALUES ('c');";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("'a;b'"));
    }

    #[test]
    fn handles_escaped_single_quotes() {
        let sql = "INSERT INTO t1 (name) VALUES ('it''s; fine'); SELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("it''s; fine"));
    }

    #[test]
    fn handles_double_quoted_identifiers_with_semicolon_and_escaped_quote() {
        let sql = r#"SELECT * FROM "weird;table" WHERE "a""b" = 1; SELECT 2;"#;
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains(r#""weird;table""#));
    }

    #[test]
    fn does_not_split_inside_line_comment() {
        let sql = "SELECT 1; -- comment with a ; semicolon\nSELECT 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn does_not_split_inside_block_comment() {
        let sql = "SELECT 1; /* comment ; with ; semicolons */ SELECT 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn does_not_split_inside_bare_dollar_quote() {
        let sql = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$\nBEGIN\n  PERFORM 1; PERFORM 2;\nEND;\n$$; SELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("PERFORM 1; PERFORM 2;"));
    }

    #[test]
    fn does_not_split_inside_tagged_dollar_quote() {
        let sql = "do $createextensions$\nbegin\n  create extension if not exists pgcrypto;\nend\n$createextensions$;\nSELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("create extension if not exists pgcrypto;"));
    }

    #[test]
    fn different_dollar_tags_do_not_close_each_other() {
        let sql = "SELECT $foo$ this has $$ inside it $foo$ as literal;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn dollar_sign_not_forming_a_quote_is_left_alone() {
        let sql = "INSERT INTO prices (amount) VALUES ('$5'); SELECT 1;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("'$5'"));
    }

    #[test]
    fn dollar_quote_tag_cannot_start_with_a_digit() {
        // `$1$` is a positional parameter then a stray `$`, not a dollar-quote opening, so
        // the `;` between the two statements must still split them.
        let sql = "SELECT $1$; SELECT 2$1$; SELECT 3;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn go_with_trailing_comment_is_a_batch_separator() {
        let sql = "CREATE TABLE t1 (id INT)\nGO -- end of batch\nCREATE TABLE t2 (id INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(
            result,
            vec!["CREATE TABLE t1 (id INT)", "CREATE TABLE t2 (id INT)"]
        );
    }

    #[test]
    fn go_with_repeat_count_repeats_the_batch() {
        let sql = "INSERT INTO t VALUES (1)\nGO 3\nSELECT 1\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(
            result,
            vec!["INSERT INTO t VALUES (1)"; 3]
                .into_iter()
                .chain(["SELECT 1"])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn go_followed_by_other_text_is_not_a_batch_separator() {
        for line in ["GO foo", "GO 0", "GO5", "GOTO label"] {
            assert_eq!(go_batch_separator_count(line), None, "{line}");
        }
        assert_eq!(go_batch_separator_count("  go  "), Some(1));
        assert_eq!(go_batch_separator_count("GO 12 -- twelve"), Some(12));
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
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
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
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
    }

    #[test]
    fn sqlite_uses_semicolon_splitting_like_postgres() {
        let sql = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
        let result = split_sql_statements(sql, &GeneratorType::Sqlite).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn sqlserver_go_inside_a_multiline_string_literal_is_not_a_separator() {
        // Regression test: a GO-only line inside a multi-line string literal must not be
        // mistaken for a real batch separator.
        let sql = "INSERT INTO notes (body) VALUES ('line one\nGO\nline three')\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("line one\nGO\nline three"));
    }

    #[test]
    fn sqlserver_go_inside_a_block_comment_is_not_a_separator() {
        // Regression test: a GO-only line inside a multi-line block comment must not be
        // mistaken for a real batch separator.
        let sql = "CREATE TABLE t1 (id INT)\n/* a comment\nGO\nstill a comment */\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("still a comment"));
    }

    #[test]
    fn sqlserver_go_after_a_multiline_string_still_splits_correctly() {
        // The batch containing the multi-line string must still end at the real,
        // top-level GO that follows it, and a second batch must start cleanly after.
        let sql = "INSERT INTO notes (body) VALUES ('a\nb')\nGO\nCREATE TABLE t2 (id INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("'a\nb'"));
        assert!(result[1].starts_with("CREATE TABLE t2"));
    }

    #[test]
    fn sqlserver_go_after_line_comment_on_same_line_still_splits_correctly() {
        let sql = "CREATE TABLE t1 (id INT) -- trailing comment\nGO\nCREATE TABLE t2 (id INT)\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].starts_with("CREATE TABLE t1"));
        assert!(result[1].starts_with("CREATE TABLE t2"));
    }

    #[test]
    fn postgres_nested_block_comment_hides_inner_statement() {
        let sql = "select 1; /* outer /* inner */ drop table users; */ select 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[1].contains("drop table users"));
        assert!(result[1].ends_with("select 2"));
    }

    #[test]
    fn sqlite_block_comments_do_not_nest() {
        let sql = "select 1; /* a /* b */ select 2; */ select 3;";
        let result = split_sql_statements(sql, &GeneratorType::Sqlite).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn sqlserver_nested_block_comment_hides_go() {
        let sql = "select 1\n/* a\n/* b */\nGO\n*/\nselect 2\nGO";
        let result = split_sql_statements(sql, &GeneratorType::SqlServer).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("select 2"));
    }

    #[test]
    fn postgres_escape_string_backslash_quote_does_not_end_literal() {
        let sql = "insert into t values (E'it\\'s; here'); select 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("it\\'s; here"));
    }

    #[test]
    fn plain_string_backslash_is_literal() {
        let sql = "select 'a\\'; select 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn trailing_e_of_identifier_is_not_an_escape_prefix() {
        let sql = "select name'a\\'; select 2;";
        let result = split_sql_statements(sql, &GeneratorType::Postgresql).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn unterminated_constructs_are_errors() {
        for (sql, db) in [
            ("select $$ oops; select 2;", GeneratorType::Postgresql),
            ("select 'oops; select 2;", GeneratorType::Postgresql),
            ("select \"oops; select 2;", GeneratorType::Sqlite),
            ("select 1; /* oops", GeneratorType::Postgresql),
            ("select 'oops\nGO", GeneratorType::SqlServer),
            ("select 1 /* /* x */\nGO", GeneratorType::SqlServer),
        ] {
            assert!(
                split_sql_statements(sql, &db).is_err(),
                "expected error for {sql:?}"
            );
        }
    }

    #[test]
    fn trailing_line_comment_without_newline_is_fine() {
        assert!(split_sql_statements("select 1; -- done", &GeneratorType::Postgresql).is_ok());
    }
}
