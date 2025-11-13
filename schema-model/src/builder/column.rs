use crate::model::column::Column;
use crate::model::column_type::ColumnType;

/// ColumnBuilder holds intermediate settings for a column and produces a model::Column on build.
#[derive(Debug)]
pub struct ColumnBuilder {
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
    ignore_case: bool,
}

impl ColumnBuilder {
    pub fn new<S: Into<String>>(schema_name: Option<S>, name: S, column_type: ColumnType) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            column_type,
            length: 0,
            scale: 0,
            required: false,
            check_constraint: None,
            default_constraint: None,
            generated: None,
            min_value: None,
            max_value: None,
            enum_type: None,
            element_type: None,
            ignore_case: false,
        }
    }
    pub fn length(mut self, length: i32) -> Self {
        self.length = length;
        self
    }

    pub fn scale(mut self, scale: i32) -> Self {
        self.scale = scale;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn check_constraint(mut self, check_constraint: Option<String>) -> Self {
        self.check_constraint = check_constraint;
        self
    }

    pub fn default_constraint(mut self, default_constraint: Option<String>) -> Self {
        self.default_constraint = default_constraint;
        self
    }

    pub fn generated(mut self, generated: Option<String>) -> Self {
        self.generated = generated;
        self
    }

    pub fn min_value(mut self, min_value: Option<f64>) -> Self {
        self.min_value = min_value;
        self
    }

    pub fn max_value(mut self, max_value: Option<f64>) -> Self {
        self.max_value = max_value;
        self
    }

    pub fn enum_type(mut self, enum_type: Option<String>) -> Self {
        self.enum_type = enum_type;
        self
    }

    pub fn element_type(mut self, element_type: Option<String>) -> Self {
        self.element_type = element_type;
        self
    }

    pub fn ignore_case(mut self, ignore_case: bool) -> Self {
        self.ignore_case = ignore_case;
        self
    }

    pub fn build(self) -> Column {
        Column::new_all(
            self.schema_name,
            self.name,
            self.column_type,
            self.length,
            self.scale,
            self.required,
            self.check_constraint,
            self.default_constraint,
            self.generated,
            self.min_value,
            self.max_value,
            self.enum_type,
            self.element_type,
            self.ignore_case,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_basic_column() {
        let c = ColumnBuilder::new(None, "name", ColumnType::Varchar)
            .length(100)
            .required(true)
            .build();
        assert_eq!(c.name(), "name");
        assert_eq!(c.length(), 100);
        assert!(c.required());
    }
}
