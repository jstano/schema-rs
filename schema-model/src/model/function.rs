use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct Function {
    schema_name: String,
    name: String,
    database_type: DatabaseType,
    sql: String,
}
impl Function {
    pub fn new<S: Into<String>>(
        schema_name: S,
        name: S,
        database_type: DatabaseType,
        sql: S,
    ) -> Self {
        Self {
            schema_name: schema_name.into(),
            name: name.into(),
            database_type,
            sql: sql.into(),
        }
    }
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::DatabaseType;

    #[test]
    fn constructor_and_getters() {
        let f = Function::new("public", "f1", DatabaseType::Postgres, "select 1");
        assert_eq!(f.schema_name(), "public");
        assert_eq!(f.name(), "f1");
        assert_eq!(f.database_type(), DatabaseType::Postgres);
        assert_eq!(f.sql(), "select 1");
    }
}
