use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct Constraint {
    name: String,
    sql: String,
    database_type: DatabaseType,
}

impl Constraint {
    pub fn new<S: Into<String>>(name: S, sql: S, database_type: DatabaseType) -> Self {
        Self {
            name: name.into(),
            sql: sql.into(),
            database_type,
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn sql(&self) -> &str { &self.sql }
    pub fn database_type(&self) -> DatabaseType { self.database_type }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::DatabaseType;

    #[test]
    fn constructor_and_getters() {
        let c = Constraint::new("ck", "check (x>0)", DatabaseType::Postgres);
        assert_eq!(c.name(), "ck");
        assert_eq!(c.sql(), "check (x>0)");
        assert_eq!(c.database_type(), DatabaseType::Postgres);
    }
}
