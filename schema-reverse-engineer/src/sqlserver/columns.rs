use super::{SqlServerClient, int, opt_string, query, string, strip_outer_parens};
use crate::error::SchemaReverseEngineerError;
use crate::info::ColumnInfo;
use schema_model::model::column_type::ColumnType;

#[derive(Debug)]
pub(crate) struct ColumnRow {
    pub table_name: String,
    pub column_name: String,
    /// Base system type name; an alias type is resolved to the type it is built on.
    pub type_name: String,
    /// Storage size in bytes (`-1` = `max`).
    pub max_length: i32,
    pub precision: i32,
    pub scale: i32,
    pub is_nullable: bool,
    pub is_identity: bool,
    pub identity_seed: Option<i64>,
    pub identity_increment: Option<i64>,
    /// Computed-column expression, as stored (wrapped in parentheses).
    pub computed_definition: Option<String>,
    pub is_persisted: bool,
    pub default_definition: Option<String>,
}

pub async fn list_columns(
    client: &mut SqlServerClient,
    db_schema: &str,
) -> Result<Vec<ColumnInfo>, SchemaReverseEngineerError> {
    let rows = query(
        client,
        "SELECT t.name AS table_name, c.name AS column_name, \
                CASE WHEN ty.is_user_defined = 1 THEN bt.name ELSE ty.name END AS type_name, \
                CAST(c.max_length AS int) AS max_length, \
                CAST(c.precision AS int) AS precision, \
                CAST(c.scale AS int) AS scale, \
                CAST(c.is_nullable AS int) AS is_nullable, \
                CAST(c.is_identity AS int) AS is_identity, \
                CAST(CAST(ic.seed_value AS bigint) AS nvarchar(40)) AS identity_seed, \
                CAST(CAST(ic.increment_value AS bigint) AS nvarchar(40)) AS identity_increment, \
                cc.definition AS computed_definition, \
                CAST(ISNULL(cc.is_persisted, 0) AS int) AS is_persisted, \
                dc.definition AS default_definition \
         FROM sys.columns c \
         JOIN sys.tables t ON t.object_id = c.object_id \
         JOIN sys.types ty ON ty.user_type_id = c.user_type_id \
         LEFT JOIN sys.types bt ON bt.user_type_id = c.system_type_id \
         LEFT JOIN sys.identity_columns ic \
                ON ic.object_id = c.object_id AND ic.column_id = c.column_id \
         LEFT JOIN sys.computed_columns cc \
                ON cc.object_id = c.object_id AND cc.column_id = c.column_id \
         LEFT JOIN sys.default_constraints dc ON dc.object_id = c.default_object_id \
         WHERE t.schema_id = SCHEMA_ID(@P1) AND t.is_ms_shipped = 0 \
         ORDER BY t.name, c.column_id",
        db_schema,
    )
    .await?;

    rows.iter()
        .map(|r| {
            Ok(build_column_info(ColumnRow {
                table_name: string(r, "table_name")?,
                column_name: string(r, "column_name")?,
                type_name: string(r, "type_name")?,
                max_length: int(r, "max_length")?,
                precision: int(r, "precision")?,
                scale: int(r, "scale")?,
                is_nullable: int(r, "is_nullable")? != 0,
                is_identity: int(r, "is_identity")? != 0,
                identity_seed: opt_string(r, "identity_seed")?.and_then(|v| v.parse().ok()),
                identity_increment: opt_string(r, "identity_increment")?
                    .and_then(|v| v.parse().ok()),
                computed_definition: opt_string(r, "computed_definition")?,
                is_persisted: int(r, "is_persisted")? != 0,
                default_definition: opt_string(r, "default_definition")?,
            }))
        })
        .collect()
}

pub(crate) fn build_column_info(row: ColumnRow) -> ColumnInfo {
    let location = format!("{}.{}", row.table_name, row.column_name);
    let type_name = row.type_name.to_ascii_lowercase();

    let mut column_type = match map_column_type(&type_name, row.precision) {
        Some(mapped) => mapped,
        None => {
            // One exotic column (xml, geography, sql_variant, ...) shouldn't abort the run.
            eprintln!(
                "warning: {location} has unsupported type '{type_name}' -- falling back to text"
            );
            ColumnType::Text
        }
    };
    if matches!(type_name.as_str(), "money" | "smallmoney") {
        eprintln!("warning: {location} is {type_name} -- exported as decimal");
    }

    if row.is_identity {
        column_type = match column_type {
            ColumnType::Int => ColumnType::Sequence,
            ColumnType::Long => ColumnType::LongSequence,
            other => {
                eprintln!(
                    "warning: {location} is an identity column of type {type_name}, which the model can't represent -- identity dropped"
                );
                other
            }
        };
        if matches!(column_type, ColumnType::Sequence | ColumnType::LongSequence)
            && (row.identity_seed.unwrap_or(1) != 1 || row.identity_increment.unwrap_or(1) != 1)
        {
            eprintln!(
                "warning: {location} is identity({},{}) -- exported as identity(1,1)",
                row.identity_seed.unwrap_or(1),
                row.identity_increment.unwrap_or(1)
            );
        }
    }

    // `varchar(max)` / `nvarchar(max)` (`max_length = -1`) and `ntext` are the model's unbounded text.
    let is_unicode = type_name.starts_with('n');
    let char_length = if row.max_length == -1 {
        -1
    } else if is_unicode {
        row.max_length / 2
    } else {
        row.max_length
    };
    if matches!(column_type, ColumnType::Char | ColumnType::Varchar) && char_length == -1 {
        column_type = ColumnType::Text;
    }

    let length = match column_type {
        ColumnType::Char | ColumnType::Varchar => char_length,
        ColumnType::Binary if row.max_length > 0 => row.max_length,
        ColumnType::Decimal => match type_name.as_str() {
            "money" => 19,
            "smallmoney" => 10,
            _ => row.precision,
        },
        _ => 0,
    };
    let scale = match column_type {
        ColumnType::Decimal if matches!(type_name.as_str(), "money" | "smallmoney") => 4,
        ColumnType::Decimal => row.scale,
        _ => 0,
    };

    let generated = row.computed_definition.as_deref().map(|definition| {
        if !row.is_persisted {
            eprintln!(
                "warning: {location} is a non-persisted computed column -- exported as persisted"
            );
        }
        strip_outer_parens(definition).to_string()
    });

    // A computed column's own default is meaningless, and the model rejects the combination.
    let default_constraint = if generated.is_some() || row.is_identity {
        None
    } else {
        row.default_definition
            .as_deref()
            .map(|d| strip_outer_parens(d).to_string())
    };

    ColumnInfo {
        table_name: row.table_name,
        column_name: row.column_name,
        column_type,
        length,
        scale,
        required: !row.is_nullable,
        default_constraint,
        generated,
        enum_type: None,
        element_type: None,
    }
}

fn map_column_type(type_name: &str, precision: i32) -> Option<ColumnType> {
    Some(match type_name {
        "int" => ColumnType::Int,
        "bigint" => ColumnType::Long,
        "smallint" => ColumnType::Short,
        "tinyint" => ColumnType::Byte,
        "bit" => ColumnType::Boolean,
        "real" => ColumnType::Float,
        // `float(n)` with n <= 24 is stored as a 4-byte real.
        "float" if precision <= 24 => ColumnType::Float,
        "float" => ColumnType::Double,
        "decimal" | "numeric" | "money" | "smallmoney" => ColumnType::Decimal,
        "char" | "nchar" => ColumnType::Char,
        "varchar" | "nvarchar" => ColumnType::Varchar,
        "text" | "ntext" => ColumnType::Text,
        "binary" | "varbinary" | "image" | "rowversion" | "timestamp" => ColumnType::Binary,
        "date" => ColumnType::Date,
        "time" => ColumnType::Time,
        "datetime" | "smalldatetime" => ColumnType::DateTime,
        "datetime2" => ColumnType::Timestamp,
        "datetimeoffset" => ColumnType::TimestampTz,
        "uniqueidentifier" => ColumnType::Uuid,
        "json" => ColumnType::Json,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(type_name: &str) -> ColumnRow {
        ColumnRow {
            table_name: "t".into(),
            column_name: "c".into(),
            type_name: type_name.into(),
            max_length: 4,
            precision: 10,
            scale: 0,
            is_nullable: true,
            is_identity: false,
            identity_seed: None,
            identity_increment: None,
            computed_definition: None,
            is_persisted: true,
            default_definition: None,
        }
    }

    #[test]
    fn identity_int_and_bigint_become_sequences() {
        let mut r = row("int");
        r.is_identity = true;
        r.identity_seed = Some(1);
        r.identity_increment = Some(1);
        assert_eq!(build_column_info(r).column_type, ColumnType::Sequence);
        let mut r = row("bigint");
        r.is_identity = true;
        assert_eq!(build_column_info(r).column_type, ColumnType::LongSequence);
    }

    #[test]
    fn nvarchar_length_is_chars_not_bytes() {
        let mut r = row("nvarchar");
        r.max_length = 200;
        let c = build_column_info(r);
        assert_eq!(c.column_type, ColumnType::Varchar);
        assert_eq!(c.length, 100);
    }

    #[test]
    fn varchar_length_is_bytes() {
        let mut r = row("varchar");
        r.max_length = 50;
        assert_eq!(build_column_info(r).length, 50);
    }

    #[test]
    fn max_text_becomes_text_with_no_length() {
        for name in ["varchar", "nvarchar"] {
            let mut r = row(name);
            r.max_length = -1;
            let c = build_column_info(r);
            assert_eq!(c.column_type, ColumnType::Text);
            assert_eq!(c.length, 0);
        }
    }

    #[test]
    fn varbinary_keeps_length_and_max_has_none() {
        let mut r = row("varbinary");
        r.max_length = 16;
        assert_eq!(build_column_info(r).length, 16);
        let mut r = row("varbinary");
        r.max_length = -1;
        assert_eq!(build_column_info(r).length, 0);
    }

    #[test]
    fn decimal_uses_precision_and_scale() {
        let mut r = row("decimal");
        r.precision = 12;
        r.scale = 3;
        let c = build_column_info(r);
        assert_eq!((c.length, c.scale), (12, 3));
    }

    #[test]
    fn float_precision_selects_float_or_double() {
        let mut r = row("float");
        r.precision = 53;
        assert_eq!(build_column_info(r).column_type, ColumnType::Double);
        let mut r = row("float");
        r.precision = 24;
        assert_eq!(build_column_info(r).column_type, ColumnType::Float);
        assert_eq!(
            build_column_info(row("real")).column_type,
            ColumnType::Float
        );
    }

    #[test]
    fn date_and_time_types_map_to_their_counterparts() {
        assert_eq!(build_column_info(row("date")).column_type, ColumnType::Date);
        assert_eq!(build_column_info(row("time")).column_type, ColumnType::Time);
        assert_eq!(
            build_column_info(row("datetime")).column_type,
            ColumnType::DateTime
        );
        assert_eq!(
            build_column_info(row("datetime2")).column_type,
            ColumnType::Timestamp
        );
        assert_eq!(
            build_column_info(row("datetimeoffset")).column_type,
            ColumnType::TimestampTz
        );
    }

    #[test]
    fn defaults_are_unwrapped_and_not_nullable_means_required() {
        let mut r = row("int");
        r.is_nullable = false;
        r.default_definition = Some("((0))".into());
        let c = build_column_info(r);
        assert!(c.required);
        assert_eq!(c.default_constraint.as_deref(), Some("0"));
    }

    #[test]
    fn computed_column_keeps_bare_expression_and_drops_default() {
        let mut r = row("int");
        r.computed_definition = Some("([a]*[b])".into());
        r.default_definition = Some("((0))".into());
        let c = build_column_info(r);
        assert_eq!(c.generated.as_deref(), Some("[a]*[b]"));
        assert_eq!(c.default_constraint, None);
    }

    #[test]
    fn unsupported_type_falls_back_to_text() {
        assert_eq!(build_column_info(row("xml")).column_type, ColumnType::Text);
        assert_eq!(
            build_column_info(row("geography")).column_type,
            ColumnType::Text
        );
    }
}
