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

Pass `--target <version>` to stop after a specific version even if later migrations exist on disk (Flyway's `-target`) — useful for staged rollouts or gating CI to a known-good version:

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  migrate --migrations-dir ./migrations --target 2
```

Running `migrate` again later with no `--target` (or a higher one) picks up the remaining migrations normally.

Pass `--dry-run <path>` to write the SQL of every pending migration to a file instead of applying anything (Flyway's `-dryRun`) — nothing is executed and nothing is recorded, so it's safe to run against a production connection for review before a real `migrate`. Combine with `--target` to preview only up to a given version:

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  migrate --migrations-dir ./migrations --dry-run ./preview.sql
```

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

`install` only records that the install itself ran; it does not mark any migration
version as applied even if `schema.xml` already reflects those changes. If you also
run `migrate` afterward with the same migrations you used to build up `schema.xml`,
it will try to re-apply all of them against a schema that already has them. Use
`--baseline-version` (with `--migrations-dir`) to mark every migration up to and
including that version as already applied, without running its SQL — Flyway calls
this `baseline`:

```bash
schema-installer \
  --database-type postgres \
  --connection-string "postgres://user:pass@localhost/mydb" \
  install --schema-file schema.xml \
    --baseline-version 3 --migrations-dir ./migrations
```

After this, `migrate` will only apply migrations with a version greater than `3`.

### Global Options

Available for all commands:

- `--database-type` (required): `postgres`, `sqlite`, or `sqlserver`
- `--connection-string` (required): Database connection URL
- `--boolean-mode`: How to represent booleans (`native`, `yesno`, `yn`) — default: `native`
- `--foreign-key-mode`: How to handle relations (`none`, `relations`, `triggers`) — default: `relations`

### Migration File Format

Files follow `V{version}__{description}.sql` format; the `__{description}` part is optional, so `V{version}.sql` is valid too:

- Versions use semantic numbering: `1`, `1.1`, `2.0`, `2.0.1`, etc.
- Versions are sorted numerically (not lexicographically): 1, 1.1, 1.10, 2.0
- Underscores in description become spaces in the UI label
- Example: `V1__create_users.sql` → "1 - create users"
- A description-less file (e.g. `V20240115143022.sql`) is treated as having an empty description — useful with timestamp-based versions where the filename doesn't need a human-readable suffix

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
