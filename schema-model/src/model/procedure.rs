use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct Procedure {
    schema_name: Option<String>,
    name: String,
    database_type: DatabaseType,
    sql: String,
}

impl Procedure {
    pub fn new<S: Into<String>>(
        schema_name: Option<S>,
        name: S,
        database_type: DatabaseType,
        sql: S,
    ) -> Self {
        Self {
            schema_name: schema_name.map(|s| s.into()),
            name: name.into(),
            database_type,
            sql: sql.into(),
        }
    }

    pub fn schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
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
        let procedure = Procedure::new(Some("public"), "p1", DatabaseType::Postgres, "begin end");
        assert_eq!(procedure.schema_name().unwrap(), "public");
        assert_eq!(procedure.name(), "p1");
        assert_eq!(procedure.database_type(), DatabaseType::Postgres);
        assert_eq!(procedure.sql(), "begin end");
    }
}
