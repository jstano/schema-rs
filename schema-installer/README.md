# schema-installer

Flyway-style migration installer for applying versioned SQL scripts to databases. Supports PostgreSQL, SQLite, and SQL Server with checksum-based migration tracking.

## Usage

### Quick Start

Create a migrations directory with SQL files:

```
migrations/
├── V1__create_users.sql
├── V1.1__add_email_index.sql
└── V2__create_posts.sql
```

Then apply them:

```bash
cargo run -p schema-installer -- \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  migrate --migrations-dir ./migrations
```

### Commands

#### `migrate` — Apply pending migrations

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  migrate --migrations-dir ./migrations
```

Applies all migrations not yet recorded in the database. Includes checksum verification to prevent applying modified migrations.

#### `info` — Display migration status

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  info --migrations-dir ./migrations
```

Shows applied and pending migrations with execution times and checksums.

#### `validate` — Verify migration integrity

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  validate --migrations-dir ./migrations
```

Checks that applied migrations haven't been modified (via SHA-256 checksum comparison).

#### `repair` — Fix broken migration state

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  repair --migrations-dir ./migrations
```

Deletes failed migrations or updates checksums after intentional edits.

#### `pending-check` — Check if migrations are pending

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  pending-check --migrations-dir ./migrations
```

Exits with code 0 if no pending migrations, 1 if migrations are pending (useful in CI).

#### `install` — Legacy XML schema installation

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  install --schema-file schema.xml
```

Applies a schema from an XML definition file (for backward compatibility).

### Global Options

Available for all commands:

- `--database-type` (required): `postgres`, `sqlite`, or `sqlserver`
- `--connection-string` (required): Database connection URL
- `--boolean-mode`: How to represent booleans (`native`, `yesno`, `yn`) — default: `native`
- `--foreign-key-mode`: How to handle relations (`none`, `relations`, `triggers`) — default: `relations`

### Migration File Format

Files follow `V{version}__{description}.sql` format:

- Versions use semantic numbering: `1`, `1.1`, `2.0`, `2.0.1`, etc.
- Versions are sorted numerically (not lexicographically): 1, 1.1, 1.10, 2.0
- Underscores in description become spaces in the UI label
- Example: `V1__create_users.sql` → "1 - create users"

### Migration Tracking

All applied migrations are recorded in the `schema_migration` table:

```sql
CREATE TABLE schema_migration (
    id BIGSERIAL PRIMARY KEY,
    version TEXT NOT NULL,
    script_path TEXT NOT NULL,
    checksum TEXT NOT NULL,              -- SHA-256 hash
    execution_time_ms INT NOT NULL,      -- Duration in milliseconds
    installed_at TIMESTAMPTZ DEFAULT now(),
    status TEXT NOT NULL,                -- success, failed, pending
    tool_version TEXT NOT NULL           -- schema-installer version
);
```

### Database Support

| Database   | Supported | Notes |
|-----------|-----------|-------|
| PostgreSQL | ✅ | Full support, uses `$1/$2` parameter placeholders |
| SQLite     | ✅ | Full support, uses `?` parameter placeholders |
| SQL Server | ✅ | Full support, uses `GO` statement separator |

## Part of schema-rs

See the [workspace README](../README.md) for an overview of the full schema-rs toolchain.
