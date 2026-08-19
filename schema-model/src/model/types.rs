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
            DatabaseType::SqlServer => 32,
        }
    }

    pub fn default_schema(&self) -> Option<&'static str> {
        match self {
            DatabaseType::Postgresql => Some("public"),
            DatabaseType::SqlServer => Some("dbo"),
            DatabaseType::Sqlite => None,
        }
    }

    pub fn qualified_name(&self, schema_name: Option<&str>, name: &str) -> String {
        let resolved = match schema_name {
            Some(s) if *self == DatabaseType::SqlServer && s.eq_ignore_ascii_case("public") => Some("dbo"),
            Some(s) => Some(s),
            None => self.default_schema(),
        };
        match resolved {
            Some(schema) => format!("{}.{}", schema, name),
            None => name.to_string(),
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
#[derive(Default)]
pub enum BooleanMode {
    #[default]
    Native,
    YesNo,
    YN,
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
#[derive(Default)]
pub enum ForeignKeyMode {
    None,
    #[default]
    Relations,
    Triggers,
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
#[derive(Default)]
pub enum LockEscalation {
    #[default]
    Auto,
    Disable,
    Table,
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

    #[test]
    fn default_schema_per_database_type() {
        assert_eq!(DatabaseType::Postgresql.default_schema(), Some("public"));
        assert_eq!(DatabaseType::SqlServer.default_schema(), Some("dbo"));
        assert_eq!(DatabaseType::Sqlite.default_schema(), None);
    }

    #[test]
    fn qualified_name_uses_default_schema_when_none() {
        assert_eq!(DatabaseType::Postgresql.qualified_name(None, "users"), "public.users");
        assert_eq!(DatabaseType::SqlServer.qualified_name(None, "users"), "dbo.users");
        assert_eq!(DatabaseType::Sqlite.qualified_name(None, "users"), "users");
    }

    #[test]
    fn qualified_name_preserves_explicit_non_default_schema() {
        assert_eq!(DatabaseType::Postgresql.qualified_name(Some("app"), "users"), "app.users");
        assert_eq!(DatabaseType::SqlServer.qualified_name(Some("app"), "users"), "app.users");
        assert_eq!(DatabaseType::Sqlite.qualified_name(Some("app"), "users"), "app.users");
    }

    #[test]
    fn qualified_name_maps_public_to_dbo_on_sql_server() {
        assert_eq!(DatabaseType::SqlServer.qualified_name(Some("public"), "users"), "dbo.users");
        assert_eq!(DatabaseType::SqlServer.qualified_name(Some("PUBLIC"), "users"), "dbo.users");
        assert_eq!(DatabaseType::Postgresql.qualified_name(Some("public"), "users"), "public.users");
    }
}
