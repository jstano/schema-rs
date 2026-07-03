# schema-parser

Parses XML schema files into the vendor-agnostic `DatabaseModel` using `roxmltree`. Re-exports the XSD schema definition for validation.

## Usage

Parse an XML schema file:

```rust
use schema_parser::parse_database_xml;
use std::fs;

let xml = fs::read_to_string("schema.xml")?;
let database_model = parse_database_xml(&xml)?;

// Access schemas, tables, columns, etc. via the model
for schema in database_model.schemas() {
    for table in schema.tables() {
        println!("Table: {}", table.name());
    }
}
```

### XML Schema Format

Supported XML elements include tables, columns, keys, indexes, relations (foreign keys), views, functions, procedures, triggers, enums, and initial data. For the complete XSD definition, see `SCHEMA_XSD` exported from this crate:

```rust
use schema_parser::SCHEMA_XSD;

println!("{}", SCHEMA_XSD);
```

A minimal example:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://stano.com/database" version="1.0">
  <table name="users">
    <columns>
      <column name="id" type="sequence" required="true"/>
      <column name="email" type="varchar" length="255" required="true"/>
    </columns>
    <keys>
      <primary>
        <column name="id"/>
      </primary>
      <unique>
        <column name="email"/>
      </unique>
    </keys>
  </table>
</database>
```

## Dependencies

- `roxmltree`: XML parsing
- `schema-model`: Core data structures
- `schema-xsd`: XSD schema definition (re-exported)

## Part of schema-rs

See the [workspace README](../README.md) for an overview of the full schema-rs toolchain.
