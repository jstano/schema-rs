use crate::model::types::RelationType;

#[derive(Debug, Clone)]
pub struct Relation {
    to_table_name: String,
    to_column_name: String,
    from_table_name: String,
    from_column_name: String,
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
            to_column_name: to_column_name.into(),
            from_table_name: from_table_name.into(),
            from_column_name: from_column_name.into(),
            relation_type,
            disable_usage_checking,
        }
    }
    pub fn to_table_name(&self) -> &str {
        &self.to_table_name
    }
    pub fn to_column_name(&self) -> &str {
        &self.to_column_name
    }
    pub fn from_table_name(&self) -> &str {
        &self.from_table_name
    }
    pub fn from_column_name(&self) -> &str {
        &self.from_column_name
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
    }
}
