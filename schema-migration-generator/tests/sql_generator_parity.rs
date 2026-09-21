//! Verifies that `schema-migration-generator`'s `ALTER TABLE ADD COLUMN` type/default
//! rendering matches `schema-sql-generator`'s `CREATE TABLE` rendering for the same column,
//! byte-for-byte, across every `ColumnType` and dialect. This is the parity check that
//! would have caught every "migration-generator drifted from schema-sql-generator" bug
//! found by hand so far (nvarchar length, enum sizing, boolean_mode, case_sensitive_text,
//! array element types, ...) - any future drift fails a test here instead of shipping.
//!
//! Black-box by design: it runs each crate's real CLI-facing generator and string-extracts
//! the column's type/default token from the rendered SQL, rather than reaching into either
//! crate's private internals.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use schema_diff::change::SchemaChange;
use schema_diff::change_set::ChangeSet;
use schema_migration_generator::create_generator;
use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::enum_type::{EnumType, EnumValue};
use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
use schema_sql_generator::common::generate_options::GenerateOptions;
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_sql_generator::common::print_writer::PrintWriter;

#[derive(Clone, Default)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedBuffer {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

/// One scenario: a single column, plus the schema-level settings that affect its rendering.
struct Scenario {
    name: &'static str,
    dialects: &'static [DatabaseType],
    boolean_mode: BooleanMode,
    case_sensitive_text: bool,
    enum_type: Option<EnumType>,
    column: Column,
}

fn scenario(name: &'static str, dialects: &'static [DatabaseType], column: Column) -> Scenario {
    Scenario {
        name,
        dialects,
        boolean_mode: BooleanMode::Native,
        case_sensitive_text: true,
        enum_type: None,
        column,
    }
}

const ALL: &[DatabaseType] = &[DatabaseType::Postgresql, DatabaseType::Sqlite, DatabaseType::SqlServer];

fn col(column_type: ColumnType) -> Column {
    ColumnBuilder::new(None::<&str>, "target", column_type).build()
}

fn scenarios() -> Vec<Scenario> {
    let mut scenarios = vec![
        scenario("sequence", ALL, col(ColumnType::Sequence)),
        scenario("long_sequence", ALL, col(ColumnType::LongSequence)),
        scenario("byte", ALL, col(ColumnType::Byte)),
        scenario("short", ALL, col(ColumnType::Short)),
        scenario("int", ALL, col(ColumnType::Int)),
        scenario("long", ALL, col(ColumnType::Long)),
        scenario("float", ALL, col(ColumnType::Float)),
        scenario("double", ALL, col(ColumnType::Double)),
        scenario("decimal_no_length", ALL, col(ColumnType::Decimal)),
        scenario(
            "decimal_length_only",
            ALL,
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Decimal).length(10).build(),
        ),
        scenario(
            "decimal_length_and_scale",
            ALL,
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Decimal).length(10).scale(2).build(),
        ),
        scenario("date", ALL, col(ColumnType::Date)),
        scenario("date_time", ALL, col(ColumnType::DateTime)),
        scenario("time", ALL, col(ColumnType::Time)),
        scenario("timestamp", ALL, col(ColumnType::Timestamp)),
        scenario("timestamp_tz", ALL, col(ColumnType::TimestampTz)),
        scenario(
            "char",
            ALL,
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Char).length(1).build(),
        ),
        scenario(
            "varchar",
            ALL,
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Varchar).length(100).build(),
        ),
        scenario("text_no_length", ALL, col(ColumnType::Text)),
        scenario(
            "text_with_length",
            &[DatabaseType::SqlServer],
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Text).length(200).build(),
        ),
        scenario("citext", ALL, col(ColumnType::CiText)),
        scenario("cstext", ALL, col(ColumnType::CsText)),
        scenario("binary_no_length", ALL, col(ColumnType::Binary)),
        scenario(
            "binary_with_length",
            &[DatabaseType::SqlServer],
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Binary).length(16).build(),
        ),
        scenario("uuid", ALL, col(ColumnType::Uuid)),
        scenario("json", ALL, col(ColumnType::Json)),
    ];

    // Boolean, all three boolean_modes.
    for mode in [BooleanMode::Native, BooleanMode::YesNo, BooleanMode::YN] {
        let mut s = scenario("boolean", ALL, col(ColumnType::Boolean));
        s.boolean_mode = mode;
        s.name = match mode {
            BooleanMode::Native => "boolean_native",
            BooleanMode::YesNo => "boolean_yesno",
            BooleanMode::YN => "boolean_yn",
        };
        scenarios.push(s);
    }

    // Text case-sensitivity only matters (differently) on Postgres.
    let mut s = scenario(
        "text_case_insensitive_schema",
        &[DatabaseType::Postgresql],
        col(ColumnType::Text),
    );
    s.case_sensitive_text = false;
    scenarios.push(s);

    // Enum: values with equal-length codes (-> char/nchar) and unequal-length codes
    // (-> varchar/nvarchar) both need coverage since `enum_sql` branches on that.
    let equal_len_enum = EnumType::new(
        "GenderType",
        vec![EnumValue::new("MALE", Some("M".to_string())), EnumValue::new("FEMALE", Some("F".to_string()))],
    );
    let mut s = scenario(
        "enum_equal_length_codes",
        ALL,
        ColumnBuilder::new(None::<&str>, "target", ColumnType::Enum)
            .enum_type(Some("GenderType".to_string()))
            .build(),
    );
    s.enum_type = Some(equal_len_enum);
    scenarios.push(s);

    let unequal_len_enum = EnumType::new(
        "StatusType",
        vec![EnumValue::new("SHORT", Some("A".to_string())), EnumValue::new("LONGER", Some("ABC".to_string()))],
    );
    let mut s = scenario(
        "enum_unequal_length_codes",
        ALL,
        ColumnBuilder::new(None::<&str>, "target", ColumnType::Enum)
            .enum_type(Some("StatusType".to_string()))
            .build(),
    );
    s.enum_type = Some(unequal_len_enum);
    scenarios.push(s);

    // Array: Postgres only - Sqlite/SqlServer's real generator panics for any array column,
    // so there's no CREATE TABLE output to compare against.
    for element_type in ["byte", "short", "int", "long", "decimal", "char", "varchar", "text"] {
        scenarios.push(scenario(
            Box::leak(format!("array_{}", element_type).into_boxed_str()),
            &[DatabaseType::Postgresql],
            ColumnBuilder::new(None::<&str>, "target", ColumnType::Array)
                .element_type(Some(element_type.to_string()))
                .build(),
        ));
    }

    scenarios
}

fn build_model(scenario: &Scenario) -> DatabaseModel {
    let table = TableBuilder::new(None::<&str>, "widget").add_column(scenario.column.clone()).build();
    let mut schema_builder = SchemaBuilder::new(None::<&str>)
        .add_table(table)
        .case_sensitive_text(scenario.case_sensitive_text);
    if let Some(enum_type) = scenario.enum_type.clone() {
        schema_builder = schema_builder.add_enum_type(enum_type);
    }
    let schema = schema_builder.build();
    DatabaseModel::new(scenario.boolean_mode, ForeignKeyMode::Relations, vec![schema])
}

/// Extracts the type token for `target` out of a rendered `CREATE TABLE` line
/// (`   target {type} not null default 'x',`) or `ALTER TABLE ... ADD [COLUMN] target {type}
/// NOT NULL DEFAULT 'x';` line. Boundaries are matched case-insensitively; the returned
/// slice keeps its original casing.
fn extract_type_token(sql: &str) -> String {
    let marker = "target ";
    let lower = sql.to_lowercase();
    let start = lower.find(marker).unwrap_or_else(|| panic!("column 'target' not found in:\n{}", sql)) + marker.len();
    let rest = &sql[start..];
    let rest_lower = &lower[start..];

    // Paren-aware: a `,` inside `decimal(10,2)` is part of the type, not a field separator.
    let boundaries = [" not null", " null", " default", ";", "\n"];
    let mut depth = 0i32;
    let mut end = rest.len();
    for (i, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
        if depth == 0 && boundaries.iter().any(|b| rest_lower[i..].starts_with(b)) {
            end = i;
            break;
        }
    }

    rest[..end].trim().to_string()
}

fn real_create_table_type(database_type: DatabaseType, model: DatabaseModel) -> String {
    let buffer = SharedBuffer::default();
    let generator_type = match database_type {
        DatabaseType::Postgresql => GeneratorType::Postgresql,
        DatabaseType::Sqlite => GeneratorType::Sqlite,
        DatabaseType::SqlServer => GeneratorType::SqlServer,
    };
    let options = GenerateOptions {
        boolean_mode: model.boolean_mode(),
        database_model: Rc::new(model),
        writer: Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
        foreign_key_mode: ForeignKeyMode::Relations,
        output_mode: schema_sql_generator::common::output_mode::OutputMode::All,
        target_postgres_version: 0,
        emit_postgres_extensions: false,
        extension_check_user: None,
    };
    generator_type.generate(options);
    extract_type_token(&buffer.contents())
}

fn migration_add_column_type(database_type: DatabaseType, model: &DatabaseModel, column: &Column) -> String {
    let mut change_set = ChangeSet::new();
    change_set.add_change(SchemaChange::AddColumn { table_name: "widget".to_string(), column: column.clone() });

    let generator = create_generator(database_type);
    let mut output = Vec::new();
    generator.generate(&change_set, model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    extract_type_token(&sql)
}

#[test]
fn migration_generator_add_column_type_matches_sql_generator_create_table_type() {
    let mut failures = Vec::new();

    for scenario in scenarios() {
        for &database_type in scenario.dialects {
            let model = build_model(&scenario);
            let column = scenario.column.clone();

            let expected = real_create_table_type(database_type, model);
            // `real_create_table_type` consumed the model; rebuild an identical one for the
            // migration-generator side.
            let model = build_model(&scenario);
            let actual = migration_add_column_type(database_type, &model, &column);

            if expected != actual {
                failures.push(format!(
                    "[{}/{:?}] expected {:?}, got {:?}",
                    scenario.name, database_type, expected, actual
                ));
            }
        }
    }

    assert!(failures.is_empty(), "type mismatches:\n{}", failures.join("\n"));
}

/// Extracts the `default` literal (e.g. `'Yes'`, `1`, `42`) from the `target` column's own
/// line, regardless of whether it's wrapped in a named constraint (`constraint df_x default
/// 1`, as `CREATE TABLE` renders it) or bare (`DEFAULT 1`, as `ALTER TABLE ADD COLUMN`
/// renders it) - both contain the literal `default ` keyword immediately before the value.
fn extract_default_token(sql: &str) -> Option<String> {
    let lower = sql.to_lowercase();
    let col_start = lower.find("target ")?;
    let line_end = lower[col_start..].find('\n').map(|i| col_start + i).unwrap_or(sql.len());
    let line = &sql[col_start..line_end];
    let line_lower = &lower[col_start..line_end];

    let marker = "default ";
    let start = line_lower.find(marker)? + marker.len();
    Some(line[start..].trim().trim_end_matches([',', ';']).to_string())
}

struct DefaultScenario {
    name: &'static str,
    dialects: &'static [DatabaseType],
    boolean_mode: BooleanMode,
    column: Column,
}

fn default_scenarios() -> Vec<DefaultScenario> {
    let bool_col = |default: &str| {
        ColumnBuilder::new(None::<&str>, "target", ColumnType::Boolean)
            .required(true)
            .default_constraint(Some(default.to_string()))
            .build()
    };

    let mut scenarios = Vec::new();
    for mode in [BooleanMode::Native, BooleanMode::YesNo, BooleanMode::YN] {
        for value in ["true", "false"] {
            scenarios.push(DefaultScenario {
                name: "boolean",
                dialects: ALL,
                boolean_mode: mode,
                column: bool_col(value),
            });
        }
    }

    // Non-boolean defaults are free-text SQL passed through unchanged by both crates - a
    // sanity check that the refactor above didn't touch that path.
    scenarios.push(DefaultScenario {
        name: "int_default",
        dialects: ALL,
        boolean_mode: BooleanMode::Native,
        column: ColumnBuilder::new(None::<&str>, "target", ColumnType::Int)
            .default_constraint(Some("42".to_string()))
            .build(),
    });
    scenarios.push(DefaultScenario {
        name: "varchar_default",
        dialects: ALL,
        boolean_mode: BooleanMode::Native,
        column: ColumnBuilder::new(None::<&str>, "target", ColumnType::Varchar)
            .length(20)
            .default_constraint(Some("'hello'".to_string()))
            .build(),
    });

    scenarios
}

fn build_default_model(scenario: &DefaultScenario) -> DatabaseModel {
    let table = TableBuilder::new(None::<&str>, "widget").add_column(scenario.column.clone()).build();
    let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
    DatabaseModel::new(scenario.boolean_mode, ForeignKeyMode::Relations, vec![schema])
}

fn real_create_table_default(database_type: DatabaseType, model: DatabaseModel) -> Option<String> {
    let buffer = SharedBuffer::default();
    let generator_type = match database_type {
        DatabaseType::Postgresql => GeneratorType::Postgresql,
        DatabaseType::Sqlite => GeneratorType::Sqlite,
        DatabaseType::SqlServer => GeneratorType::SqlServer,
    };
    let options = GenerateOptions {
        boolean_mode: model.boolean_mode(),
        database_model: Rc::new(model),
        writer: Rc::new(RefCell::new(PrintWriter::new_auto_flush(Box::new(buffer.clone())))),
        foreign_key_mode: ForeignKeyMode::Relations,
        output_mode: schema_sql_generator::common::output_mode::OutputMode::All,
        target_postgres_version: 0,
        emit_postgres_extensions: false,
        extension_check_user: None,
    };
    generator_type.generate(options);
    extract_default_token(&buffer.contents())
}

fn migration_add_column_default(database_type: DatabaseType, model: &DatabaseModel, column: &Column) -> Option<String> {
    let mut change_set = ChangeSet::new();
    change_set.add_change(SchemaChange::AddColumn { table_name: "widget".to_string(), column: column.clone() });

    let generator = create_generator(database_type);
    let mut output = Vec::new();
    generator.generate(&change_set, model, &mut output).unwrap();
    let sql = String::from_utf8(output).unwrap();
    extract_default_token(&sql)
}

#[test]
fn migration_generator_add_column_default_matches_sql_generator_create_table_default() {
    let mut failures = Vec::new();

    for scenario in default_scenarios() {
        for &database_type in scenario.dialects {
            let model = build_default_model(&scenario);
            let column = scenario.column.clone();

            let expected = real_create_table_default(database_type, model);
            let model = build_default_model(&scenario);
            let actual = migration_add_column_default(database_type, &model, &column);

            if expected != actual {
                failures.push(format!(
                    "[{}/{:?}] expected {:?}, got {:?}",
                    scenario.name, database_type, expected, actual
                ));
            }
        }
    }

    assert!(failures.is_empty(), "default mismatches:\n{}", failures.join("\n"));
}
