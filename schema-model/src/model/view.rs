use crate::model::types::DatabaseType;
use std::fmt;

#[derive(Debug, Clone)]
pub struct View {
    schema_name: Option<String>,
    name: String,
    sql: String,
    database_type: Option<DatabaseType>,
}
impl View {
    pub fn new<S: Into<String>>(
        schema_name: Option<S>,
        name: S,
        sql: S,
        database_type: Option<DatabaseType>,
    ) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            sql: sql.into(),
            database_type,
        }
    }
    pub fn schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn database_type(&self) -> Option<DatabaseType> {
        self.database_type
    }

    pub fn fully_qualified_view_name(&self) -> String {
        match self.schema_name() {
            Some(schema_name) => format!("{}.{}", schema_name, self.name()),
            None => self.name().to_string(),
        }
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.schema_name {
            Some(schema) => write!(f, "{}.{}", schema, self.name),
            None => write!(f, "{}", self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_getters_and_display() {
        let v = View::new(Some("s"), "v1", "select *", Some(DatabaseType::Postgres));
        assert_eq!(v.schema_name().unwrap(), "s");
        assert_eq!(v.name(), "v1");
        assert_eq!(v.sql(), "select *");
        assert_eq!(v.database_type().unwrap(), DatabaseType::Postgres);
        assert_eq!(format!("{}", v), "s.v1");
    }
}
