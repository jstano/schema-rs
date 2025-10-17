#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    // Extend as needed
    Postgres,
    Mysql,
    Sqlite,
    SqlServer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationType {
    Cascade,
    Enforce,
    SetNull,
    DoNothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BooleanMode {
    Native,
    YesNo,
    YN,
}

impl Default for BooleanMode {
    fn default() -> Self {
        BooleanMode::Native
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForeignKeyMode {
    None,
    Relations,
    Triggers,
}

impl Default for ForeignKeyMode {
    fn default() -> Self {
        ForeignKeyMode::Relations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OtherSqlOrder {
    Bottom,
    Top,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TableOption {
    Data,
    NoExport,
    Compress
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerType {
    Update,
    Delete
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    Primary,
    Unique,
    Index
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockEscalation {
    Auto,
}

// Re-export Version so external crates can access it via model::types
pub use crate::model::version::Version;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_mode_default_is_native() {
        let bm = BooleanMode::default();
        assert_eq!(bm, BooleanMode::Native);
    }

    #[test]
    fn enums_equality_and_copy() {
        // Just ensure variants compare and copy derives exist
        let db: DatabaseType = DatabaseType::Postgres;
        let db2 = db; // Copy
        assert_eq!(db, db2);

        let t1 = TriggerType::Update;
        let t2 = TriggerType::Update;
        assert_eq!(t1, t2);

        let rel = RelationType::Cascade;
        assert_eq!(rel, RelationType::Cascade);

        let k = KeyType::Primary;
        assert_eq!(k, KeyType::Primary);

        let fk = ForeignKeyMode::Relations;
        assert_eq!(fk, ForeignKeyMode::Relations);

        let ord = OtherSqlOrder::Top;
        assert_eq!(ord, OtherSqlOrder::Top);

        let le = LockEscalation::Auto;
        assert_eq!(le, LockEscalation::Auto);
    }

    #[test]
    fn table_option_equality() {
        assert_eq!(TableOption::Compress, TableOption::Compress);
        assert_ne!(TableOption::Data, TableOption::NoExport);
    }
}

