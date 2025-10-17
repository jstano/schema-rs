#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnType {
    Sequence,
    LongSequence,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    Decimal,
    Boolean,
    Date,
    DateTime,
    Time,
    Timestamp,
    Char,
    Varchar,
    Enum,
    Text,
    Binary,
    Uuid,
    Json,
    Array,
}
impl ColumnType {
    pub const VARIANTS: [ColumnType; 22] = [
        ColumnType::Sequence,
        ColumnType::LongSequence,
        ColumnType::Byte,
        ColumnType::Short,
        ColumnType::Int,
        ColumnType::Long,
        ColumnType::Float,
        ColumnType::Double,
        ColumnType::Decimal,
        ColumnType::Boolean,
        ColumnType::Date,
        ColumnType::DateTime,
        ColumnType::Time,
        ColumnType::Timestamp,
        ColumnType::Char,
        ColumnType::Varchar,
        ColumnType::Enum,
        ColumnType::Text,
        ColumnType::Binary,
        ColumnType::Uuid,
        ColumnType::Json,
        ColumnType::Array,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ColumnType::Sequence => "SEQUENCE",
            ColumnType::LongSequence => "LONGSEQUENCE",
            ColumnType::Byte => "BYTE",
            ColumnType::Short => "SHORT",
            ColumnType::Int => "INT",
            ColumnType::Long => "LONG",
            ColumnType::Float => "FLOAT",
            ColumnType::Double => "DOUBLE",
            ColumnType::Decimal => "DECIMAL",
            ColumnType::Boolean => "BOOLEAN",
            ColumnType::Date => "DATE",
            ColumnType::DateTime => "DATETIME",
            ColumnType::Time => "TIME",
            ColumnType::Timestamp => "TIMESTAMP",
            ColumnType::Char => "CHAR",
            ColumnType::Varchar => "VARCHAR",
            ColumnType::Enum => "ENUM",
            ColumnType::Text => "TEXT",
            ColumnType::Binary => "BINARY",
            ColumnType::Uuid => "UUID",
            ColumnType::Json => "JSON",
            ColumnType::Array => "ARRAY",
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(
            self,
            ColumnType::Char
                | ColumnType::Varchar
                | ColumnType::Enum
                | ColumnType::Text
                | ColumnType::Json
                | ColumnType::Uuid
        )
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            ColumnType::Sequence
                | ColumnType::LongSequence
                | ColumnType::Byte
                | ColumnType::Short
                | ColumnType::Int
                | ColumnType::Long
                | ColumnType::Float
                | ColumnType::Double
                | ColumnType::Decimal
        )
    }

    pub fn from_type_name(type_name: &str) -> Result<Self, String> {
        let upper = type_name.trim().to_uppercase();
        Self::VARIANTS
            .iter()
            .copied()
            .find(|v| v.name() == upper)
            .ok_or_else(|| format!("The type '{}' is not valid.", type_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_roundtrip_and_from_type_name() {
        for v in ColumnType::VARIANTS.iter() {
            let name = v.name();
            let parsed = ColumnType::from_type_name(name).expect("must parse");
            assert_eq!(*v, parsed);
        }
        // Lower/space handling
        assert_eq!(ColumnType::from_type_name(" int ").unwrap(), ColumnType::Int);
        // Error case
        let err = ColumnType::from_type_name("notatype").unwrap_err();
        assert!(err.contains("notatype"));
    }

    #[test]
    fn is_text_and_numeric_classification() {
        assert!(ColumnType::Varchar.is_text());
        assert!(ColumnType::Text.is_text());
        assert!(!ColumnType::Int.is_text());

        assert!(ColumnType::Int.is_numeric());
        assert!(ColumnType::Decimal.is_numeric());
        assert!(!ColumnType::Varchar.is_numeric());
    }
}
