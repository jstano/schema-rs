# schema-model

Core data structures for database schemas. Defines a vendor-agnostic model to represent tables, columns, relations, views, functions, procedures, triggers, constraints, and more.

## Usage

The model uses a **builder pattern** to construct schema objects. Here's a simple example:

```rust
use schema_model::builder::{ColumnBuilder, TableBuilder, SchemaBuilder};
use schema_model::model::types::ColumnType;

let table = TableBuilder::new(None, "users")
    .add_column(
        ColumnBuilder::new(None, "id", ColumnType::Int)
            .required(true)
            .build()
    )
    .add_column(
        ColumnBuilder::new(None, "name", ColumnType::Varchar)
            .length(100)
            .required(true)
            .build()
    )
    .build();

let schema = SchemaBuilder::new(None::<&str>)
    .add_table(table)
    .build();
```

## Features

- **DatabaseModel**: Top-level container for all schemas, versions, and configuration (foreign key mode, boolean representation, etc.)
- **Schema**: Groups tables, views, functions, procedures, enums, and custom SQL by schema name
- **Table**: Columns, keys (primary, unique), indexes, relations (foreign keys), triggers, constraints, and initial data
- **Column Types**: Sequence, LongSequence, Byte, Short, Int, Long, Float, Double, Decimal, Boolean, Date, DateTime, Time, Timestamp, Char, Varchar, Enum, Text, Binary, Uuid, Json, Array
- **Relations**: Foreign key constraints with modes: Cascade, Enforce, SetNull, DoNothing
- **EnumType**: Stored per schema; columns reference them by name. Each enum value has both a human-readable `name` and optional compact `code`
- **Case-Insensitive Lookups**: Tables and columns are accessible via case-insensitive HashMap index within their parent
- **Multi-Schema Support**: Default schema fallback for vendor portability

## Part of schema-rs

See the [workspace README](../README.md) for an overview of the full schema-rs toolchain.
