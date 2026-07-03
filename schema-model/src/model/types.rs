#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    Postgresql,
    Sqlite,
    SqlServer,
}

impl DatabaseType {
    pub fn statement_separator(&self) -> &'static str {
        match self {
            DatabaseType::Postgresql => ";",
            DatabaseType::Sqlite => ";",
            DatabaseType::SqlServer => "\nGO",
        }
    }

    pub fn max_key_name_length(&self) -> usize {
        match self {
            DatabaseType::Postgresql => 63,
            DatabaseType::Sqlite => 128,
            DatabaseType::SqlServer => 128,
        }
    }
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

impl FromStr for BooleanMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "native" => Ok(BooleanMode::Native),
            "yesno" => Ok(BooleanMode::YesNo),
            "yn" => Ok(BooleanMode::YN),
            _ => Err(format!("Unknown boolean mode: {}", s)),
        }
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

impl FromStr for ForeignKeyMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(ForeignKeyMode::None),
            "relations" => Ok(ForeignKeyMode::Relations),
            "triggers" => Ok(ForeignKeyMode::Triggers),
            _ => Err(format!("Unknown foreign key mode: {}", s)),
        }
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
    Compress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerType {
    Update,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    Primary,
    Unique,
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockEscalation {
    Auto,
    Disable,
    Table,
}

impl Default for LockEscalation {
    fn default() -> Self {
        LockEscalation::Auto
    }
}

impl FromStr for LockEscalation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(LockEscalation::Auto),
            "disable" => Ok(LockEscalation::Disable),
            "table" => Ok(LockEscalation::Table),
            _ => Err(format!("Unknown lock escalation: {}", s)),
        }
    }
}

// Re-export Version so external crates can access it via model::types
pub use crate::model::version::Version;
use std::str::FromStr;

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
        let db: DatabaseType = DatabaseType::Postgresql;
        let db2 = db;
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
