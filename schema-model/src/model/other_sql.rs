use crate::model::types::{DatabaseType, OtherSqlOrder};

#[derive(Debug, Clone)]
pub struct OtherSql {
    database_type: DatabaseType,
    order: OtherSqlOrder,
    sql: String,
}
impl OtherSql {
    pub fn new<S: Into<String>>(database_type: DatabaseType, order: OtherSqlOrder, sql: S) -> Self {
        Self {
            database_type,
            order,
            sql: sql.into(),
        }
    }
    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }
    pub fn order(&self) -> OtherSqlOrder {
        self.order
    }
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_getters() {
        let o = OtherSql::new(DatabaseType::Mysql, OtherSqlOrder::Bottom, "SQL");
        assert_eq!(o.database_type(), DatabaseType::Mysql);
        assert_eq!(o.order(), OtherSqlOrder::Bottom);
        assert_eq!(o.sql(), "SQL");
    }
}
