use schema_installer::SchemaInstallerConfigBuilder;
use schema_sql_generator::common::generator_type::GeneratorType;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use std::path::PathBuf;
use std::fs;
use std::env;

const SIMPLE_SCHEMA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://stano.com/database" version="1.0">
    <table name="users">
        <columns>
            <column name="id" type="Sequence" required="true"/>
            <column name="name" type="Varchar" length="100" required="true"/>
            <column name="email" type="Varchar" length="100"/>
        </columns>
        <keys>
            <primary>
                <column name="id"/>
            </primary>
        </keys>
    </table>
</database>"#;

fn create_test_schema() -> PathBuf {
    let temp_dir = env::temp_dir();
    let schema_path = temp_dir.join("test-schema.xml");
    fs::write(&schema_path, SIMPLE_SCHEMA).expect("Failed to write test schema");
    schema_path
}

#[test]
fn test_schema_installer_config_builder() {
    let schema_path = create_test_schema();

    // Test that config builder validates all required fields
    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string("sqlite::memory:".to_string())
        .schema_file(schema_path.clone())
        .boolean_mode(BooleanMode::Native)
        .foreign_key_mode(ForeignKeyMode::Relations)
        .build();

    assert!(config.is_ok(), "Config build should succeed");
    let cfg = config.unwrap();
    assert_eq!(cfg.boolean_mode, BooleanMode::Native);
    assert_eq!(cfg.foreign_key_mode, ForeignKeyMode::Relations);

    let _ = fs::remove_file(schema_path);
}

#[test]
fn test_schema_installer_config_missing_database_type() {
    let schema_path = create_test_schema();

    // Test that config builder fails without database_type
    let config = SchemaInstallerConfigBuilder::new()
        .connection_string("sqlite::memory:".to_string())
        .schema_file(schema_path.clone())
        .build();

    assert!(config.is_err());

    let _ = fs::remove_file(schema_path);
}

#[test]
fn test_schema_installer_config_missing_connection_string() {
    let schema_path = create_test_schema();

    // Test that config builder fails without connection_string
    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .schema_file(schema_path.clone())
        .build();

    assert!(config.is_err());

    let _ = fs::remove_file(schema_path);
}

#[test]
fn test_schema_installer_config_missing_schema_file() {
    // Test that config builder fails without schema_file
    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string("sqlite::memory:".to_string())
        .build();

    assert!(config.is_err());
}

#[test]
fn test_schema_installer_config_nonexistent_schema_file() {
    // Test that config builder fails with nonexistent schema file
    let config = SchemaInstallerConfigBuilder::new()
        .database_type(GeneratorType::Sqlite)
        .connection_string("sqlite::memory:".to_string())
        .schema_file(PathBuf::from("/nonexistent/path/schema.xml"))
        .build();

    assert!(config.is_err());
}
