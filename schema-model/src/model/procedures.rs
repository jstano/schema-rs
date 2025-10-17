use crate::model::procedure::Procedure;
use crate::model::types::DatabaseType;

#[derive(Debug, Clone)]
pub struct Procedures {
    database_type: DatabaseType,
    procedures: Vec<Procedure>,
}

impl Procedures {
    pub fn new(database_type: DatabaseType, procedures: Vec<Procedure>) -> Self {
        // In Java this is wrapped as an unmodifiable copy; here we take ownership of the Vec
        // and expose only immutable access via getter.
        Self { database_type, procedures }
    }

    pub fn database_type(&self) -> DatabaseType { self.database_type }

    pub fn procedures(&self) -> &Vec<Procedure> { &self.procedures }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::procedure::Procedure;

    #[test]
    fn constructor_and_getters() {
        let ps = vec![
            Procedure::new("s", "a", DatabaseType::Sqlite, ""),
            Procedure::new("s", "b", DatabaseType::Sqlite, ""),
        ];
        let pset = Procedures::new(DatabaseType::Sqlite, ps);
        assert_eq!(pset.database_type(), DatabaseType::Sqlite);
        assert_eq!(pset.procedures().len(), 2);
        assert_eq!(pset.procedures()[0].name(), "a");
    }
}
