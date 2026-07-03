# schema-sql-generator

Generates vendor-specific SQL CREATE statements from a `DatabaseModel`. Supports PostgreSQL, MySQL, SQL Server, SQLite, and H2 with customizable boolean and foreign key handling.

## Usage

### CLI

```bash
cargo run -p schema-sql-generator -- \
  --database-type postgres \
  --schema-file schema.xml \
  --foreign-key-mode relations \
  --boolean-mode native \
  --output-mode all \
  --pg-version 17
```

**Arguments:**

- `--database-type` (required): `postgres`, `mysql`, `sqlserver`, `sqlite`, or `h2`
- `--schema-file` (required): Path to XML schema file
- `--foreign-key-mode` (optional): How to represent foreign keys — `none`, `relations`, or `triggers` (default: `relations`)
- `--boolean-mode` (optional): Boolean column representation — `native`, `yesno`, or `yn` (default: `native`)
- `--output-mode` (optional): What to generate — `all`, `indexes-only`, or `triggers-only` (default: `all`)
- `--pg-version` (optional): Target PostgreSQL version (e.g. 17, 18); affects UUID generation function

**Output:**

Writes SQL to a file named `{schema-stem}-{database-type}.sql` in the same directory as the input schema file. For example, `schema.xml` → `schema-postgres.sql`.

### Architecture

Uses the **Strategy Pattern** with trait-based abstraction:

- **Component Traits**: `TableGenerator`, `ColumnTypeGenerator`, `ColumnConstraintGenerator`, `RelationGenerator`, `FunctionGenerator`, `ProcedureGenerator`, `TriggerGenerator`, `ViewGenerator`, `IndexGenerator`, `OtherSqlGenerator`
- **Database Implementations**: Each database folder (`postgresql/`, `mysql/`, etc.) overrides only the traits that differ from common defaults
- **Shared Context**: `GeneratorContext` wraps settings and SQL writer, passed by reference to all component generators

## Database-Specific Behavior

| Database   | Sequence Type | String Type | UUID Support | Array Support |
|-----------|---------------|-------------|--------------|---------------|
| PostgreSQL | `SERIAL` / `BIGSERIAL` | `text` or `citext` | RFC 4122 v7 function | Yes |
| MySQL      | `INTEGER AUTO_INCREMENT` | `VARCHAR` | `CHAR(36)` with `UUID()` | No |
| SQL Server | `INT IDENTITY(1,1)` | `NVARCHAR` | `CHAR(36)` | No |
| SQLite     | `INTEGER PRIMARY KEY AUTOINCREMENT` | `TEXT` | `TEXT` | No |
| H2         | Hybrid (mimics multiple databases) | Vendor-aware | Vendor-aware | Limited |

## Part of schema-rs

See the [workspace README](../README.md) for an overview of the full schema-rs toolchain.
