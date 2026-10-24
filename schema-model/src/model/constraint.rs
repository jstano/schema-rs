use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct Constraint {
    name: String,
    sql: String,
    /// `None` means the constraint applies to every database.
    database_type: Option<DatabaseType>,
}

impl Constraint {
    pub fn new<S: Into<String>>(name: S, sql: S, database_type: DatabaseType) -> Self {
        Self {
            name: name.into(),
            sql: sql.into(),
            database_type: Some(database_type),
        }
    }

    /// A portable constraint, emitted for every database.
    pub fn for_every_database<S: Into<String>>(name: S, sql: S) -> Self {
        Self {
            name: name.into(),
            sql: sql.into(),
            database_type: None,
        }
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
    pub fn applies_to(&self, database_type: DatabaseType) -> bool {
        self.database_type.is_none_or(|d| d == database_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::DatabaseType;

    #[test]
    fn constructor_and_getters() {
        let c = Constraint::new("ck", "check (x>0)", DatabaseType::Postgresql);
        assert_eq!(c.name(), "ck");
        assert_eq!(c.sql(), "check (x>0)");
        assert_eq!(c.database_type(), Some(DatabaseType::Postgresql));
        assert!(c.applies_to(DatabaseType::Postgresql));
        assert!(!c.applies_to(DatabaseType::Sqlite));
    }

    #[test]
    fn portable_constraint_applies_to_every_database() {
        let c = Constraint::for_every_database("ck", "check (x>0)");
        assert_eq!(c.database_type(), None);
        assert!(c.applies_to(DatabaseType::Postgresql));
        assert!(c.applies_to(DatabaseType::Sqlite));
        assert!(c.applies_to(DatabaseType::SqlServer));
    }
}
