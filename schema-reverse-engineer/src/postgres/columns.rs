use crate::error::SchemaReverseEngineerError;
use schema_model::model::column_type::ColumnType;
use sqlx::PgPool;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub table_name: String,
    pub column_name: String,
    pub column_type: ColumnType,
    pub length: i32,
    pub scale: i32,
    pub required: bool,
    pub default_constraint: Option<String>,
    pub generated: Option<String>,
    pub enum_type: Option<String>,
    pub element_type: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ColumnRow {
    table_name: String,
    column_name: String,
    data_type: String,
    udt_name: String,
    character_maximum_length: Option<i32>,
    numeric_precision: Option<i32>,
    numeric_scale: Option<i32>,
    is_nullable: String,
    column_default: Option<String>,
    is_identity: String,
    is_generated: String,
    generation_expression: Option<String>,
}

pub async fn list_columns(
    pool: &PgPool,
    db_schema: &str,
    enum_type_names: &HashSet<String>,
) -> Result<Vec<ColumnInfo>, SchemaReverseEngineerError> {
    let rows: Vec<ColumnRow> = sqlx::query_as(
        "SELECT c.table_name, c.column_name, c.data_type, c.udt_name, \
                c.character_maximum_length::int4 AS character_maximum_length, \
                c.numeric_precision::int4 AS numeric_precision, \
                c.numeric_scale::int4 AS numeric_scale, \
                c.is_nullable, c.column_default, c.is_identity, c.is_generated, c.generation_expression \
         FROM information_schema.columns c \
         JOIN information_schema.tables t \
           ON t.table_schema = c.table_schema AND t.table_name = c.table_name AND t.table_type = 'BASE TABLE' \
         WHERE c.table_schema = $1 \
         ORDER BY c.table_name, c.ordinal_position",
    )
    .bind(db_schema)
    .fetch_all(pool)
    .await
    .map_err(|e| SchemaReverseEngineerError::Introspection(e.to_string()))?;

    rows.into_iter()
        .map(|row| build_column_info(row, enum_type_names))
        .collect()
}

fn build_column_info(row: ColumnRow, enum_type_names: &HashSet<String>) -> Result<ColumnInfo, SchemaReverseEngineerError> {
    let is_generated = row.is_generated == "ALWAYS";
    let is_autoincrement = row.is_identity == "YES"
        || row
            .column_default
            .as_deref()
            .map(|d| d.starts_with("nextval("))
            .unwrap_or(false);

    let (column_type, element_type) =
        map_column_type(&row.data_type, &row.udt_name, is_autoincrement, enum_type_names)?;

    let enum_type = if column_type == ColumnType::Enum {
        Some(row.udt_name.clone())
    } else {
        None
    };

    let length = match column_type {
        ColumnType::Varchar | ColumnType::Char | ColumnType::CiText => row.character_maximum_length.unwrap_or(0),
        ColumnType::Decimal => row.numeric_precision.unwrap_or(0),
        _ => 0,
    };
    let scale = if column_type == ColumnType::Decimal {
        row.numeric_scale.unwrap_or(0)
    } else {
        0
    };

    let generated = if is_generated {
        Some(format!(
            "generated always as ({}) stored",
            row.generation_expression.unwrap_or_default()
        ))
    } else {
        None
    };

    // Postgres always reports a default expression for identity/sequence columns
    // (e.g. `nextval(...)`); suppress it here since the column type itself already
    // conveys that it's auto-generated (matches the Java writer's behavior).
    let default_constraint = if is_generated || matches!(column_type, ColumnType::Sequence | ColumnType::LongSequence) {
        None
    } else {
        row.column_default
    };

    Ok(ColumnInfo {
        table_name: row.table_name,
        column_name: row.column_name,
        column_type,
        length,
        scale,
        required: row.is_nullable == "NO",
        default_constraint,
        generated,
        enum_type,
        element_type,
    })
}

/// Maps a Postgres column's `data_type`/`udt_name` (as reported by `information_schema.columns`)
/// to a schema-model `ColumnType`, plus an element-type name (lowercase `ColumnType::name()`)
/// when the column is an array.
///
/// Unlike the Java original -- which relied on JDBC `java.sql.Types` and crashed with
/// `IllegalArgumentException` on `Types.OTHER` (Postgres `uuid`, `jsonb`, and native enum columns
/// all surface that way via JDBC) -- this maps directly off Postgres's own catalog type names, so
/// `uuid`/`jsonb`/enum/array columns are all handled without special-casing a JDBC quirk.
pub fn map_column_type(
    data_type: &str,
    udt_name: &str,
    is_autoincrement: bool,
    enum_type_names: &HashSet<String>,
) -> Result<(ColumnType, Option<String>), SchemaReverseEngineerError> {
    if data_type == "ARRAY" {
        let element_udt = udt_name.strip_prefix('_').unwrap_or(udt_name);
        let (element_column_type, _) = base_type_to_column_type(element_udt, false, enum_type_names)?;
        return Ok((ColumnType::Array, Some(element_column_type.name().to_lowercase())));
    }

    base_type_to_column_type(udt_name, is_autoincrement, enum_type_names)
}

fn base_type_to_column_type(
    udt_name: &str,
    is_autoincrement: bool,
    enum_type_names: &HashSet<String>,
) -> Result<(ColumnType, Option<String>), SchemaReverseEngineerError> {
    let column_type = match udt_name {
        "int4" => {
            if is_autoincrement {
                ColumnType::Sequence
            } else {
                ColumnType::Int
            }
        }
        "int8" => {
            if is_autoincrement {
                ColumnType::LongSequence
            } else {
                ColumnType::Long
            }
        }
        "int2" => ColumnType::Short,
        "float4" => ColumnType::Float,
        "float8" => ColumnType::Double,
        "numeric" => ColumnType::Decimal,
        "bool" => ColumnType::Boolean,
        "varchar" => ColumnType::Varchar,
        "bpchar" => ColumnType::Char,
        "text" => ColumnType::Text,
        "citext" => ColumnType::CiText,
        "bytea" => ColumnType::Binary,
        "date" => ColumnType::Date,
        // Postgres has no dedicated "time with timezone" model type; `Time` is the closest
        // available representation (the Java original mapped this to `TIMESTAMPTZ`, which was
        // very likely a bug -- a time-only value has no date component to make a timestamp of).
        "time" | "timetz" => ColumnType::Time,
        "timestamp" => ColumnType::DateTime,
        "timestamptz" => ColumnType::TimestampTz,
        "uuid" => ColumnType::Uuid,
        "json" | "jsonb" => ColumnType::Json,
        other if enum_type_names.contains(other) => ColumnType::Enum,
        other => return Err(SchemaReverseEngineerError::UnsupportedColumnType(other.to_string())),
    };
    Ok((column_type, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enums(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn maps_uuid_and_jsonb() {
        let none = enums(&[]);
        assert_eq!(map_column_type("uuid", "uuid", false, &none).unwrap().0, ColumnType::Uuid);
        assert_eq!(map_column_type("jsonb", "jsonb", false, &none).unwrap().0, ColumnType::Json);
        assert_eq!(map_column_type("USER-DEFINED", "json", false, &none).unwrap().0, ColumnType::Json);
    }

    #[test]
    fn maps_native_enum_type() {
        let mood = enums(&["mood"]);
        let (column_type, element_type) = map_column_type("USER-DEFINED", "mood", false, &mood).unwrap();
        assert_eq!(column_type, ColumnType::Enum);
        assert_eq!(element_type, None);
    }

    #[test]
    fn maps_array_of_text() {
        let none = enums(&[]);
        let (column_type, element_type) = map_column_type("ARRAY", "_text", false, &none).unwrap();
        assert_eq!(column_type, ColumnType::Array);
        assert_eq!(element_type.as_deref(), Some("text"));
    }

    #[test]
    fn maps_array_of_int4() {
        let none = enums(&[]);
        let (column_type, element_type) = map_column_type("ARRAY", "_int4", false, &none).unwrap();
        assert_eq!(column_type, ColumnType::Array);
        assert_eq!(element_type.as_deref(), Some("int"));
    }

    #[test]
    fn maps_sequence_for_autoincrement_int4() {
        let none = enums(&[]);
        assert_eq!(
            map_column_type("int4", "int4", true, &none).unwrap().0,
            ColumnType::Sequence
        );
        assert_eq!(
            map_column_type("int8", "int8", true, &none).unwrap().0,
            ColumnType::LongSequence
        );
    }

    #[test]
    fn maps_numeric_to_decimal() {
        let none = enums(&[]);
        assert_eq!(map_column_type("numeric", "numeric", false, &none).unwrap().0, ColumnType::Decimal);
    }

    #[test]
    fn unknown_type_is_an_error() {
        let none = enums(&[]);
        let err = map_column_type("USER-DEFINED", "some_domain", false, &none).unwrap_err();
        assert!(matches!(err, SchemaReverseEngineerError::UnsupportedColumnType(_)));
    }
}
