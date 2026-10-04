use crate::common::generator_context::GeneratorContext;
use schema_model::model::view::View;
use std::collections::HashSet;

pub trait ViewGenerator {
    fn output_views(&self);
}

/// Splits `sql` into lowercase identifier-like tokens (letters, digits, `_`, `$`), so
/// quoting (`"v"`, `[v]`, `` `v` ``) and schema qualification (`s.v`) don't hide a name.
fn identifier_tokens(sql: &str) -> HashSet<String> {
    sql.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

/// Orders `views` so that a view whose SQL references another view comes after it (M9):
/// emitting in declaration order fails when a view selects from a later-declared view.
///
/// Dependencies are found by matching each view's (case-insensitive) name against the
/// identifiers in the other views' SQL, since view bodies are opaque SQL. Views with no
/// dependency between them keep their declaration order. Views caught in a dependency
/// cycle fall back to declaration order after everything orderable - no ordering
/// satisfies a cycle, and this matches the pre-fix behavior.
pub fn views_in_dependency_order(views: Vec<View>) -> Vec<View> {
    let count = views.len();
    let tokens: Vec<HashSet<String>> = views
        .iter()
        .map(|view| identifier_tokens(view.sql()))
        .collect();
    let names: Vec<String> = views
        .iter()
        .map(|view| view.name().to_lowercase())
        .collect();

    // dependencies[i] = indexes of the views that view i references.
    let dependencies: Vec<Vec<usize>> = (0..count)
        .map(|i| {
            (0..count)
                .filter(|&j| j != i && tokens[i].contains(&names[j]))
                .collect()
        })
        .collect();

    let mut emitted = vec![false; count];
    let mut order = Vec::with_capacity(count);

    while let Some(next) =
        (0..count).find(|&i| !emitted[i] && dependencies[i].iter().all(|&d| emitted[d]))
    {
        emitted[next] = true;
        order.push(next);
    }

    order.extend((0..count).filter(|&i| !emitted[i]));

    let mut slots: Vec<Option<View>> = views.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

pub struct DefaultViewGenerator {
    context: GeneratorContext,
}

impl DefaultViewGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl ViewGenerator for DefaultViewGenerator {
    fn output_views(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str, sql: &str) -> View {
        View::new(None::<&str>, name, sql, None)
    }

    fn names(views: &[View]) -> Vec<&str> {
        views.iter().map(|v| v.name()).collect()
    }

    #[test]
    fn view_selecting_from_later_declared_view_is_moved_after_it() {
        let ordered = views_in_dependency_order(vec![
            view("b", "select * from a"),
            view("a", "select 1 from users"),
        ]);
        assert_eq!(names(&ordered), vec!["a", "b"]);
    }

    #[test]
    fn independent_views_keep_declaration_order() {
        let ordered = views_in_dependency_order(vec![view("z", "select 1"), view("a", "select 2")]);
        assert_eq!(names(&ordered), vec!["z", "a"]);
    }

    #[test]
    fn handles_chains_quoting_qualification_and_case() {
        let ordered = views_in_dependency_order(vec![
            view("c", "select * from public.\"B\""),
            view("b", "select * from [dbo].A"),
            view("a", "select 1"),
        ]);
        assert_eq!(names(&ordered), vec!["a", "b", "c"]);
    }

    #[test]
    fn name_inside_a_longer_identifier_is_not_a_dependency() {
        let ordered = views_in_dependency_order(vec![
            view("x", "select * from a_users"),
            view("a", "select 1"),
        ]);
        assert_eq!(names(&ordered), vec!["x", "a"]);
    }

    #[test]
    fn cycle_falls_back_to_declaration_order() {
        let ordered = views_in_dependency_order(vec![
            view("a", "select * from b"),
            view("b", "select * from a"),
            view("c", "select 1"),
        ]);
        assert_eq!(names(&ordered), vec!["c", "a", "b"]);
    }
}
