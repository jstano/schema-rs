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
        }
    }
    pub fn length(mut self, len: i32) -> Self {
        self.length = len;
        self
    }
    pub fn scale(mut self, sc: i32) -> Self {
        self.scale = sc;
        self
    }
    pub fn required(mut self, r: bool) -> Self {
        self.required = r;
        self
    }

    pub fn build(self) -> Column {
        Column::new(
            self.schema_name,
            self.name,
            self.column_type,
            self.length,
            self.scale,
            self.required,
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
