use super::{SqlServerClient, opt_string, query, string};
use crate::error::SchemaReverseEngineerError;
use schema_model::model::types::DatabaseType;
use schema_model::model::view::View;

pub async fn list_views(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<Vec<View>, SchemaReverseEngineerError> {
    // `definition` is NULL for an encrypted view.
    let rows = query(
        client,
        "SELECT v.name AS name, m.definition AS definition \
         FROM sys.views v \
         JOIN sys.sql_modules m ON m.object_id = v.object_id \
         WHERE v.schema_id = SCHEMA_ID(@P1) AND v.is_ms_shipped = 0 \
         ORDER BY v.name",
        db_schema,
    )
    .await?;

    let mut views = Vec::new();
    for row in &rows {
        let name = string(row, "name")?;
        let Some(definition) = opt_string(row, "definition")? else {
            eprintln!(
                "warning: definition of view {name} is not visible (encrypted?) -- skipping it"
            );
            continue;
        };
        match view_body(&definition) {
            Some(body) => views.push(View::new(
                None,
                name.as_str(),
                body.as_str(),
                Some(DatabaseType::SqlServer),
            )),
            None => eprintln!(
                "warning: couldn't find the SELECT in the definition of view {name} -- skipping it"
            ),
        }
    }
    Ok(views)
}

/// The query of a view's `CREATE VIEW <name> [(columns)] [WITH ...] AS <query>`. The model
/// stores only the query (the generator writes the `create view` header), so anything between
/// the name and `AS` -- a column list or `WITH SCHEMABINDING` -- is dropped with a warning.
pub(crate) fn view_body(definition: &str) -> Option<String> {
    let as_at = find_top_level_as(definition)?;
    let header = &definition[..as_at];
    let extra = header_extras(header);
    if extra {
        eprintln!(
            "warning: a view header has a column list or WITH options that the model can't represent -- exporting the query only"
        );
    }
    let body = definition[as_at + 2..].trim();
    (!body.is_empty()).then(|| body.to_string())
}

/// True when the header (`CREATE VIEW <name> ...` up to `AS`) holds more than the name.
fn header_extras(header: &str) -> bool {
    let tokens = header
        .split_whitespace()
        .skip_while(|t| t.eq_ignore_ascii_case("create") || t.eq_ignore_ascii_case("alter"))
        .skip(1) // VIEW
        .count();
    tokens > 1
}

/// Byte offset of the first `AS` keyword that is outside parentheses, quotes and brackets.
fn find_top_level_as(sql: &str) -> Option<usize> {
    let bytes = sql.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else {
            match b {
                b'\'' | b'"' => quote = Some(b),
                b'[' => quote = Some(b']'),
                b'(' => depth += 1,
                b')' => depth -= 1,
                b'a' | b'A'
                    if depth == 0
                        && i + 1 < bytes.len()
                        && bytes[i + 1].eq_ignore_ascii_case(&b's')
                        && (i == 0 || !is_ident(bytes[i - 1]))
                        && (i + 2 >= bytes.len() || !is_ident(bytes[i + 2])) =>
                {
                    return Some(i);
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'@' || b == b'#' || b == b'$'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_select_after_as() {
        assert_eq!(
            view_body("CREATE VIEW [dbo].[v] AS\r\nSELECT a FROM t").as_deref(),
            Some("SELECT a FROM t")
        );
    }

    #[test]
    fn an_as_inside_the_name_or_query_is_not_the_separator() {
        assert_eq!(
            view_body("CREATE VIEW [dbo].[as_view] AS SELECT a AS b FROM t").as_deref(),
            Some("SELECT a AS b FROM t")
        );
        assert_eq!(
            view_body("create view [has as in it] as select 1").as_deref(),
            Some("select 1")
        );
    }

    #[test]
    fn with_options_are_dropped_from_the_header() {
        assert_eq!(
            view_body("CREATE VIEW v WITH SCHEMABINDING AS SELECT a FROM dbo.t").as_deref(),
            Some("SELECT a FROM dbo.t")
        );
    }

    #[test]
    fn column_list_with_as_in_parens_is_skipped_over() {
        assert_eq!(
            view_body("CREATE VIEW v (x, y) AS SELECT a, b FROM t").as_deref(),
            Some("SELECT a, b FROM t")
        );
    }

    #[test]
    fn no_as_means_no_body() {
        assert_eq!(view_body("CREATE VIEW v"), None);
    }
}
