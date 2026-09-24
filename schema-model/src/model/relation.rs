use crate::model::types::RelationType;

#[derive(Debug, Clone)]
pub struct Relation {
    to_table_name: String,
    from_table_name: String,
    column_pairs: Vec<(String, String)>,
    relation_type: RelationType,
    disable_usage_checking: bool,
}
impl Relation {
    #[allow(clippy::too_many_arguments)]
    pub fn new<SS: Into<String>>(
        to_table_name: SS,
        to_column_name: SS,
        from_table_name: SS,
        from_column_name: SS,
        relation_type: RelationType,
        disable_usage_checking: bool,
    ) -> Self {
        Self {
            to_table_name: to_table_name.into(),
            from_table_name: from_table_name.into(),
            column_pairs: vec![(from_column_name.into(), to_column_name.into())],
            relation_type,
            disable_usage_checking,
        }
    }

    /// Constructs a composite (multi-column) foreign key relation from a list of
    /// `(from_column_name, to_column_name)` pairs, one per column position.
    ///
    /// Errors (rather than silently coercing) when fewer than two pairs are given -
    /// a single-pair "composite" relation should be constructed via [`Relation::new`] instead.
    pub fn new_composite<SS: Into<String>>(
        to_table_name: SS,
        from_table_name: SS,
        column_pairs: Vec<(SS, SS)>,
        relation_type: RelationType,
        disable_usage_checking: bool,
    ) -> Result<Self, String> {
        if column_pairs.len() < 2 {
            return Err(format!(
                "composite relation must have at least 2 column pairs, got {}",
                column_pairs.len()
            ));
        }
        Ok(Self {
            to_table_name: to_table_name.into(),
            from_table_name: from_table_name.into(),
            column_pairs: column_pairs
                .into_iter()
                .map(|(from, to)| (from.into(), to.into()))
                .collect(),
            relation_type,
            disable_usage_checking,
        })
    }

    pub fn to_table_name(&self) -> &str {
        &self.to_table_name
    }

    /// First column pair's target-table column. For a composite relation, prefer
    /// [`Relation::column_pairs`] to see every mapped column.
    pub fn to_column_name(&self) -> &str {
        &self.column_pairs[0].1
    }

    pub fn from_table_name(&self) -> &str {
        &self.from_table_name
    }

    /// First column pair's local column. For a composite relation, prefer
    /// [`Relation::column_pairs`] to see every mapped column.
    pub fn from_column_name(&self) -> &str {
        &self.column_pairs[0].0
    }

    /// All `(from_column_name, to_column_name)` pairs, in declared order.
    pub fn column_pairs(&self) -> &[(String, String)] {
        &self.column_pairs
    }

    pub fn is_composite(&self) -> bool {
        self.column_pairs.len() > 1
    }

    pub fn relation_type(&self) -> RelationType {
        self.relation_type
    }

    pub fn disable_usage_checking(&self) -> bool {
        self.disable_usage_checking
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::RelationType;

    #[test]
    fn constructor_and_getters() {
        let r = Relation::new(
            "parent",
            "id",
            "child",
            "parent_id",
            RelationType::Cascade,
            true,
        );
        assert_eq!(r.to_table_name(), "parent");
        assert_eq!(r.to_column_name(), "id");
        assert_eq!(r.from_table_name(), "child");
        assert_eq!(r.from_column_name(), "parent_id");
        assert_eq!(r.relation_type(), RelationType::Cascade);
        assert!(r.disable_usage_checking());
        assert!(!r.is_composite());
        assert_eq!(
            r.column_pairs(),
            &[("parent_id".to_string(), "id".to_string())]
        );
    }

    #[test]
    fn composite_constructor_and_getters() {
        let r = Relation::new_composite(
            "Assignment",
            "Child",
            vec![("ParentAssignmentID", "ID"), ("PropertyID", "PropertyID")],
            RelationType::Cascade,
            false,
        )
        .unwrap();
        assert_eq!(r.to_table_name(), "Assignment");
        assert_eq!(r.from_table_name(), "Child");
        assert!(r.is_composite());
        assert_eq!(r.from_column_name(), "ParentAssignmentID");
        assert_eq!(r.to_column_name(), "ID");
        assert_eq!(
            r.column_pairs(),
            &[
                ("ParentAssignmentID".to_string(), "ID".to_string()),
                ("PropertyID".to_string(), "PropertyID".to_string()),
            ]
        );
        assert_eq!(r.relation_type(), RelationType::Cascade);
        assert!(!r.disable_usage_checking());
    }

    #[test]
    fn composite_constructor_rejects_empty_pairs() {
        let err = Relation::new_composite::<&str>(
            "Assignment",
            "Child",
            vec![],
            RelationType::Cascade,
            false,
        )
        .unwrap_err();
        assert!(err.contains("at least 2"));
    }

    #[test]
    fn composite_constructor_rejects_single_pair() {
        let err = Relation::new_composite(
            "Assignment",
            "Child",
            vec![("PropertyID", "PropertyID")],
            RelationType::Cascade,
            false,
        )
        .unwrap_err();
        assert!(err.contains("at least 2"));
    }
}
