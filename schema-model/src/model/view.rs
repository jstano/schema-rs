use std::fmt;
use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct View {
    schema_name: String,
    name: String,
    sql: String,
    database_type: DatabaseType,
}
impl View {
    pub fn new<S: Into<String>>(schema_name: S, name: S, sql: S, database_type: DatabaseType) -> Self {
        Self {
            schema_name: schema_name.into(),
            name: name.into(),
            sql: sql.into(),
            database_type,
        }
    }
    pub fn schema_name(&self) -> &str { &self.schema_name }
    pub fn name(&self) -> &str { &self.name }
    pub fn sql(&self) -> &str { &self.sql }
    pub fn database_type(&self) -> DatabaseType { self.database_type }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "View({}.{})", self.schema_name, self.name) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_getters_and_display() {
        let v = View::new("s", "v1", "select *", DatabaseType::Postgres);
        assert_eq!(v.schema_name(), "s");
        assert_eq!(v.name(), "v1");
        assert_eq!(v.sql(), "select *");
        assert_eq!(v.database_type(), DatabaseType::Postgres);
        assert_eq!(format!("{}", v), "View(s.v1)");
    }
}
