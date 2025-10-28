use crate::model::column_type::ColumnType;
use crate::model::types::BooleanMode;

#[derive(Debug, Clone)]
pub struct Column {
    schema_name: Option<String>,
    name: String,
    column_type: ColumnType,
    length: i32,
    scale: i32,
    required: bool,
    check_constraint: Option<String>,
    default_constraint: Option<String>,
    generated: Option<String>,
    min_value: Option<String>,
    max_value: Option<String>,
    enum_type: Option<String>,
    element_type: Option<String>,
    unicode: bool,
    ignore_case: bool,
}

impl Column {
    pub fn new<S: Into<String>>(
        schema_name: Option<S>,
        name: S,
        column_type: ColumnType,
        length: i32,
        scale: i32,
        required: bool,
    ) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            column_type,
            length,
            scale,
            required,
            check_constraint: None,
            default_constraint: None,
            generated: None,
            min_value: None,
            max_value: None,
            enum_type: None,
            element_type: None,
            unicode: false,
            ignore_case: false,
        }
    }

    pub fn schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn column_type(&self) -> ColumnType {
        self.column_type
    }

    pub fn length(&self) -> i32 {
        self.length
    }

    pub fn scale(&self) -> i32 {
        self.scale
    }

    pub fn required(&self) -> bool {
        self.required
    }

    // Compatibility alias: elsewhere in the codebase `is_required()` is used.
    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn check_constraint(&self) -> Option<&str> {
        self.check_constraint.as_deref()
    }

    pub fn default_constraint(&self) -> Option<&str> {
        self.default_constraint.as_deref()
    }

    pub fn generated(&self) -> Option<&str> {
        self.generated.as_deref()
    }

    pub fn min_value(&self) -> Option<&str> {
        self.min_value.as_deref()
    }

    pub fn max_value(&self) -> Option<&str> {
        self.max_value.as_deref()
    }

    pub fn enum_type(&self) -> Option<&str> {
        self.enum_type.as_deref()
    }

    pub fn element_type(&self) -> Option<&str> {
        self.element_type.as_deref()
    }

    pub fn unicode(&self) -> bool {
        self.unicode
    }

    pub fn ignore_case(&self) -> bool {
        self.ignore_case
    }

    pub fn has_min_or_max_value(&self) -> bool {
        self.min_value.is_some() || self.max_value.is_some()
    }

    pub fn needs_check_constraints(&self, boolean_mode: BooleanMode) -> bool {
        self.check_constraint.is_some()
            || self.min_value.is_some()
            || self.max_value.is_some()
            || self.enum_type.is_some()
            || (self.column_type == ColumnType::Boolean && boolean_mode != BooleanMode::Native)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::BooleanMode;

    #[test]
    fn constructor_and_getters() {
        let c = Column::new(None, "name", ColumnType::Varchar, 255, 0, true);
        assert_eq!(c.name(), "name");
        assert_eq!(c.column_type(), ColumnType::Varchar);
        assert_eq!(c.length(), 255);
        assert_eq!(c.scale(), 0);
        assert!(c.required());
        assert!(c.is_required());
        assert!(!c.unicode());
        assert!(!c.ignore_case());
    }

    #[test]
    fn needs_check_constraints_logic() {
        let c = Column::new(None, "b", ColumnType::Boolean, 0, 0, false);
        // boolean with non-native boolean mode => needs constraints
        assert!(c.needs_check_constraints(BooleanMode::YesNo));
        // boolean with native => no unless other attributes set
        assert!(!c.needs_check_constraints(BooleanMode::Native));
    }

    #[test]
    fn has_min_or_max_value() {
        let c = Column::new(None, "n", ColumnType::Int, 0, 0, false);
        assert!(!c.has_min_or_max_value());
    }
}
