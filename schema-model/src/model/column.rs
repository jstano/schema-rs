use crate::model::column_type::ColumnType;
use crate::model::types::BooleanMode;

/// Parses a boolean column's `default` attribute (an untyped `xsd:string` in the schema XSD,
/// not `xsd:boolean` - it has to tolerate whatever a hand-written schema file spells out) into
/// either an explicit default value or, for the `null` sentinel, an explicit request for *no*
/// default constraint to be emitted at all (distinct from the attribute being absent, which
/// callers should handle before ever calling this).
///
/// Recognizes `true`/`1`/`yes`/`on` and `false`/`0`/`no`/`off`, trimmed and case-insensitive.
/// Returns `Err(())` for anything else - shared by `Schema::validate()` (which reports it as a
/// validation error up front) and the SQL generator (which can then treat any value reaching it
/// as already validated, rather than silently guessing).
pub fn parse_boolean_default(value: &str) -> Result<Option<bool>, ()> {
    let trimmed = value.trim();

    if trimmed.eq_ignore_ascii_case("null") {
        return Ok(None);
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(Some(true)),
        "false" | "0" | "no" | "off" => Ok(Some(false)),
        _ => Err(()),
    }
}

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
    min_value: Option<f64>,
    max_value: Option<f64>,
    enum_type: Option<String>,
    element_type: Option<String>,
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
        }
    }

    pub fn new_all<S: Into<String>>(
        schema_name: Option<S>,
        name: S,
        column_type: ColumnType,
        length: i32,
        scale: i32,
        required: bool,
        check_constraint: Option<String>,
        default_constraint: Option<String>,
        generated: Option<String>,
        min_value: Option<f64>,
        max_value: Option<f64>,
        enum_type: Option<String>,
        element_type: Option<String>,
    ) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            column_type,
            length,
            scale,
            required,
            check_constraint,
            default_constraint,
            generated,
            min_value,
            max_value,
            enum_type,
            element_type,
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

    pub fn min_value(&self) -> Option<f64> {
        self.min_value
    }

    pub fn max_value(&self) -> Option<f64> {
        self.max_value
    }

    pub fn enum_type(&self) -> Option<&str> {
        self.enum_type.as_deref()
    }

    pub fn element_type(&self) -> Option<&str> {
        self.element_type.as_deref()
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
    }

    #[test]
    fn parse_boolean_default_recognizes_truthy_spellings() {
        for value in ["true", "TRUE", " true ", "1", "yes", "YES", "on"] {
            assert_eq!(parse_boolean_default(value), Ok(Some(true)), "value: {:?}", value);
        }
    }

    #[test]
    fn parse_boolean_default_recognizes_falsy_spellings() {
        for value in ["false", "FALSE", " false ", "0", "no", "NO", "off"] {
            assert_eq!(parse_boolean_default(value), Ok(Some(false)), "value: {:?}", value);
        }
    }

    #[test]
    fn parse_boolean_default_null_sentinel_means_no_default() {
        assert_eq!(parse_boolean_default("null"), Ok(None));
        assert_eq!(parse_boolean_default("NULL"), Ok(None));
        assert_eq!(parse_boolean_default(" null "), Ok(None));
    }

    #[test]
    fn parse_boolean_default_rejects_unrecognized_values() {
        for value in ["maybe", "T", "getdate()", ""] {
            assert_eq!(parse_boolean_default(value), Err(()), "value: {:?}", value);
        }
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
