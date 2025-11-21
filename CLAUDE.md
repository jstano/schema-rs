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
- 20+ column types: Sequence, Int, Long, Varchar, Boolean, UUID, JSON, Array, etc.
- Relation types: Cascade, Enforce, SetNull, DoNothing

### SQL Generator Architecture

The generator uses **Strategy Pattern** with trait-based abstraction:

**Component Traits** (in `common/`):
- `SqlGenerator` - Main orchestrator, defines output order
- `TableGenerator` - CREATE TABLE statements
- `RelationGenerator` - Foreign key constraints
- `FunctionGenerator` - CREATE FUNCTION
- `ProcedureGenerator` - CREATE PROCEDURE
- `TriggerGenerator` - CREATE TRIGGER
- `ViewGenerator` - CREATE VIEW
- `IndexGenerator` - CREATE INDEX
- `OtherSqlGenerator` - Custom SQL

**Database-Specific Implementations** (in `{database}/`):
- Each database has a folder: `mysql/`, `postgresql/`, `sqlite/`, `sqlserver/`, `h2/`
- Each implements all component generator traits
- Example: `postgresql/postgres_table_generator.rs`, `mysql/mysql_function_generator.rs`

**Shared Context:**
- `GeneratorContext` wraps `SqlGeneratorSettings` and `SqlWriter`
- Uses `Rc<RefCell<>>` for shared mutable state across generators
- Passed to all component generators for access to configuration

**SQL Generation Flow:**
1. `GeneratorType::generate()` factory creates database-specific generator
2. `DefaultSqlGenerator` orchestrates component generators in order:
   - output_header()
   - output_other_sql_top()
   - output_tables()
   - output_relations() [if ForeignKeyMode::Relations]
   - output_triggers()
   - output_functions()
   - output_views()
   - output_procedures()
   - output_other_sql_bottom()

### Database-Specific Behaviors

**PostgreSQL:**
- Sequence: `SERIAL` / `BIGSERIAL`
- UUID: Custom RFC 4122 v7 generator function
- Extensions: uuid-ossp, citext, btree_gist
- Array support: `type[]` syntax

**MySQL:**
- Sequence: `INTEGER AUTO_INCREMENT`
- UUID: `CHAR(36)`, uses `UUID()` function
- Text: `MEDIUMTEXT`, Binary: `MEDIUMBLOB`
- No array support (panics)

**SQL Server:**
- Sequence: `INT IDENTITY(1,1)`
- Unicode: Always generates `NVARCHAR` (unicode flag removed)
- Statement separator: `GO` instead of `;`

**SQLite:**
- Sequence: `INTEGER PRIMARY KEY AUTOINCREMENT`
- Limited type support (pragmatic mapping)

**H2:**
- Hybrid approach mimicking multiple databases

## Testing

**Integration Tests:**
- Parser tests in `schema-parser/tests/parser_integration_tests.rs`
- Test resources in `schema-parser/resources/`
- Example schemas: `schema-parser-test-schema.xml`, `schema-parser-example-schema.xml`

**Test Data:**
- XML schemas define multi-table test databases
- Generated SQL output in resources: `schema-parser-test-schema.sql`, etc.

## Adding a New Database

1. Create folder: `schema-sql-generator/src/{dbname}/`
2. Implement all generator traits:
   - `{dbname}_table_generator.rs`
   - `{dbname}_column_type_generator.rs`
   - `{dbname}_relation_generator.rs`
   - `{dbname}_function_generator.rs`
   - `{dbname}_procedure_generator.rs`
   - `{dbname}_trigger_generator.rs`
   - `{dbname}_view_generator.rs`
   - `{dbname}_index_generator.rs`
   - `{dbname}_other_sql_generator.rs`
   - `{dbname}_table_constraint_generator.rs`
3. Override `column_type_sql()` in column type generator for type mappings
4. Add variant to `GeneratorType` enum in `common/generator_type.rs`
5. Update CLI parser in `main.rs` to accept new database type

## Extending the Schema Model

1. Add structs to `schema-model/src/model/` (e.g., `new_entity.rs`)
2. Update parent container (`DatabaseModel`, `Schema`, or `Table`)
3. Add parser logic in `schema-parser/src/parser/convert.rs` or create new parser module
4. Update XML schema definition (`schema.xsd` in resources)
5. Implement generator trait in `common/` folder
6. Override in database-specific folders as needed

## Configuration Options

**GenerateOptions** (in `schema-sql-generator`):
- `database_model`: Shared Rc<RefCell<DatabaseModel>>
- `writer`: Output sink (file or stdout)
- `boolean_mode`: Native | YesNo | YN
- `foreign_key_mode`: None | Relations | Triggers
- `output_mode`: All | IndexesOnly | TriggersOnly

**Edition:** Rust 2024

## Key Implementation Notes

- **SQL Server Unicode:** Always generates `NVARCHAR` (unicode flag removed in recent commit)
- **Relations Generator:** Implemented in recent commits, removes unicode flag
- **Parser:** Switched from custom parser to roxmltree for safer XML handling
- **Index Generation:** Separate from table constraints, generated after relations
- **Case Sensitivity:** Model uses case-insensitive lookups for table/column names
