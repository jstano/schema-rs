use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct InitialData {
    sql: String,
    database_type: Option<DatabaseType>,
}

impl InitialData {
    pub fn new<S: Into<String>>(sql: S, database_type: Option<DatabaseType>) -> Self {
        Self {
            sql: sql.into(),
            database_type,
        }
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }
    pub fn database_type(&self) -> Option<DatabaseType> {
        self.database_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_getters() {
        let i = InitialData::new("insert into t values (1)", Option::from(DatabaseType::Postgres));
        assert_eq!(i.sql(), "insert into t values (1)");
        assert_eq!(i.database_type(), Option::from(DatabaseType::Postgres));
    }
}
