use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct InitialData {
    sql: String,
    database_type: DatabaseType,
}

impl InitialData {
    pub fn new<S: Into<String>>(sql: S, database_type: DatabaseType) -> Self {
        Self { sql: sql.into(), database_type }
    }

    pub fn sql(&self) -> &str { &self.sql }
    pub fn database_type(&self) -> DatabaseType { self.database_type }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_getters() {
        let i = InitialData::new("insert into t values (1)", DatabaseType::Postgres);
        assert_eq!(i.sql(), "insert into t values (1)");
        assert_eq!(i.database_type(), DatabaseType::Postgres);
    }
}
