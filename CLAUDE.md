# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

schema-rs is a Rust workspace for managing relational database schemas. It provides:
- A vendor-agnostic schema model (schema-model)
- XML schema parser (schema-parser)
- Multi-database SQL generator (schema-sql-generator)
- Schema installer utilities (schema-installer, WIP)

Supported databases: PostgreSQL, MySQL, SQL Server, SQLite, H2

## Build & Test Commands

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p schema-model
cargo test -p schema-parser
cargo test -p schema-sql-generator

# Run a single test
cargo test test_parser -p schema-parser

# Check for errors without building
cargo check --workspace

# Lint
cargo clippy --workspace

# Format code
cargo fmt --workspace

# Generate documentation
cargo doc --workspace --open

# Run SQL generator (requires schema file)
cargo run -p schema-sql-generator -- \
  --database-type postgres \
  --schema-file schema-parser/resources/schema-parser-test-schema.xml \
  --foreign-key-mode relations \
  --boolean-mode native

# Generate code coverage (macOS)
cargo make coverage
```

The CLI writes output to a file named `{schema-stem}-{database-type}.sql` in the same directory as the input schema file (e.g. `schema-parser-test-schema-postgres.sql`), not to stdout.

## Architecture

### Workspace Structure

```
schema-model          → Core data structures (Table, Column, Relation, etc.)
schema-parser         → XML → DatabaseModel parser (uses roxmltree)
schema-sql-generator  → DatabaseModel → SQL generator (multi-database)
schema-installer      → Schema installation utilities (WIP)
```

### Data Flow

```
XML Schema File → [schema-parser] → DatabaseModel → [schema-sql-generator] → SQL
```

### Schema Model Hierarchy

The core model in `schema-model/src/model/`:

```
DatabaseModel
├── version: Option<Version>
├── foreign_key_mode: ForeignKeyMode
├── boolean_mode: BooleanMode
└── schemas: Vec<Schema>
    ├── schema_name: Option<String>  (None = default schema)
    ├── tables: Vec<Table>
    │   ├── columns: Vec<Column>
    │   ├── keys: Vec<Key>           (primary, unique)
    │   ├── indexes: Vec<Key>
    │   ├── relations: Vec<Relation> (foreign keys)
    │   ├── triggers: Vec<Trigger>
    │   ├── constraints: Vec<Constraint>
    │   └── initial_data: Vec<InitialData>
    ├── views: Vec<View>
    ├── functions: Vec<Function>
    ├── procedures: Vec<Procedure>
    ├── enum_types: HashMap<String, EnumType>
    └── other_sql: Vec<OtherSql>
```

**Key Features:**
- Case-insensitive table/column lookups via HashMap index
- Multi-schema support with default schema fallback
- Validation for SETNULL relations on required columns
- Column types: Sequence, LongSequence, Byte, Short, Int, Long, Float, Double, Decimal, Boolean, Date, DateTime, Time, Timestamp, Char, Varchar, Enum, Text, Binary, Uuid, Json, Array
- Relation types: Cascade, Enforce, SetNull, DoNothing

**Builder Pattern:** Model objects are constructed via builders in `schema-model/src/builder/`. Use `ColumnBuilder`, `TableBuilder`, `SchemaBuilder`, etc. rather than calling constructors directly.

```rust
ColumnBuilder::new(schema_name, "col_name", ColumnType::Varchar)
    .length(100)
    .required(true)
    .build()
```

**EnumType:** `EnumValue` has both a `name` (human-readable label, e.g. `"MALE"`) and an optional `code` (compact storage value, e.g. `"M"`). `code()` falls back to `name` when not set. Enum types live on `Schema`, not `Table`; columns reference them by name via `Column::enum_type: Option<String>`.

### SQL Generator Architecture

The generator uses **Strategy Pattern** with trait-based abstraction:

**Component Traits** (in `common/`):
- `SqlGenerator` - Main orchestrator, defines output order
- `TableGenerator` - CREATE TABLE statements
- `ColumnTypeGenerator` - Maps `ColumnType` to database-specific SQL type strings
- `ColumnConstraintGenerator` - CHECK constraints per column
- `RelationGenerator` - Foreign key constraints
- `FunctionGenerator` - CREATE FUNCTION
- `ProcedureGenerator` - CREATE PROCEDURE
- `TriggerGenerator` - CREATE TRIGGER
- `ViewGenerator` - CREATE VIEW
- `IndexGenerator` - CREATE INDEX
- `OtherSqlGenerator` - Custom SQL passthrough

**Database-Specific Implementations** (in `{database}/`):
- Each database has a folder: `mysql/`, `postgresql/`, `sqlite/`, `sqlserver/`, `h2/`
- Each overrides only the trait methods that differ from the common defaults
- Example: `postgresql/postgres_table_generator.rs`, `mysql/mysql_function_generator.rs`

**Shared Context:**
- `GeneratorContext` wraps `Rc<SqlGeneratorSettings>` and `Rc<RefCell<SqlWriter>>`
- Cloned cheaply (reference-counted) and passed to all component generators
- Access settings via `context.settings()`, write SQL via `context.with_writer(|w| { ... })`

**Writing SQL in generators** — use the macros from `sql_writer.rs`:

```rust
self.context.with_writer(|writer| {
    sql_println!(writer, "create table {} (", table.name());
    sql_println!(writer, "   {} integer not null,", col.name());
    sql_println!(writer, ");");
    sql_println!(writer, "");
});
```

**SQL Generation Flow:**
1. `GeneratorType::generate()` factory creates the database-specific generator
2. `DefaultSqlGenerator` orchestrates component generators in order:
   - `output_header()`
   - `output_other_sql_top()`
   - `output_tables()`
   - `output_relations()` [if ForeignKeyMode::Relations]
   - `output_triggers()`
   - `output_functions()`
   - `output_views()`
   - `output_procedures()`
   - `output_other_sql_bottom()`

### Database-Specific Behaviors

**PostgreSQL:**
- Sequence: `SERIAL` / `BIGSERIAL`
- UUID: Custom RFC 4122 v7 generator function emitted in `output_header()`
- Extensions: uuid-ossp, citext, btree_gist (emitted in `output_header()`)
- `varchar` → `text` (or `citext` when `ignore_case = true`)
- Array support: `type[]` syntax

**MySQL:**
- Sequence: `INTEGER AUTO_INCREMENT`
- UUID: `CHAR(36)`, uses `UUID()` function
- Text: `MEDIUMTEXT`, Binary: `MEDIUMBLOB`
- No array support (panics)

**SQL Server:**
- Sequence: `INT IDENTITY(1,1)`
- Always generates `NVARCHAR` for string types
- Statement separator: `GO` instead of `;`

**SQLite:**
- Sequence: `INTEGER PRIMARY KEY AUTOINCREMENT`
- Limited type support (pragmatic mapping)

**H2:**
- Hybrid approach mimicking multiple databases

## Testing

**Unit tests** live in the same file as the code they test (e.g. `column_type.rs`, `builder/column.rs`).

**Integration tests:**
- Parser tests: `schema-parser/tests/parser_integration_tests.rs`
- Test resources: `schema-parser/resources/`
- Reference XML schemas: `schema-parser-test-schema.xml`, `schema-parser-example-schema.xml`
- The `.sql` files in `schema-parser/resources/` are generated SQL output checked in as reference; there are no automated SQL output comparison tests.

## Adding a New Database

1. Create folder: `schema-sql-generator/src/{dbname}/`
2. Implement all generator traits:
   - `{dbname}_table_generator.rs`
   - `{dbname}_column_type_generator.rs`
   - `{dbname}_column_constraint_generator.rs`
   - `{dbname}_relation_generator.rs`
   - `{dbname}_function_generator.rs`
   - `{dbname}_procedure_generator.rs`
   - `{dbname}_trigger_generator.rs`
   - `{dbname}_view_generator.rs`
   - `{dbname}_index_generator.rs`
   - `{dbname}_other_sql_generator.rs`
   - `{dbname}_table_constraint_generator.rs`
3. In the column type generator, override the specific `{type}_sql()` methods that differ from the common defaults (e.g. `sequence_sql()`, `text_sql()`, `binary_sql()`). The `column_type_sql()` dispatch method should not be overridden.
4. Add variant to `GeneratorType` enum in `common/generator_type.rs`
5. Update CLI parser in `main.rs` to accept the new database type string

## Extending the Schema Model

1. Add structs to `schema-model/src/model/`
2. Update parent container (`DatabaseModel`, `Schema`, or `Table`)
3. Add parser logic in `schema-parser/src/parser/convert.rs` or create new parser module
4. Update XML schema definition (`schema.xsd` in resources)
5. Implement a generator trait in `common/` folder
6. Override in database-specific folders as needed

## Configuration Options

**GenerateOptions** (in `schema-sql-generator`):
- `database_model`: Shared `Rc<RefCell<DatabaseModel>>`
- `writer`: Output sink (file or stdout)
- `boolean_mode`: `Native` | `YesNo` | `YN`
- `foreign_key_mode`: `None` | `Relations` | `Triggers`
- `output_mode`: `All` | `IndexesOnly` | `TriggersOnly`

**Edition:** Rust 2024
