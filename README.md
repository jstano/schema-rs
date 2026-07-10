# schema-rs

A small workspace that models database schemas (schema-model), parses/handles inputs (schema-parser), and can generate SQL (schema-sql-generator) or install a schema (schema-installer).

## FAQ

- Q: Is there a Rust crate that implements all of this for me?
  
  A: There isn’t a single crate that does exactly what this repository is aiming for (a compact model + parser + SQL generator + installer following this specific design). However, depending on your needs, the following projects can cover large parts of the problem:
  
  - SeaQuery: A builder-style library to programmatically construct SQL; often used by SeaORM. Useful if you want to generate portable SQL without hand-writing it.
  - SeaORM Migrations: Structured migrations integrated with SeaORM that can generate and run SQL for supported databases.
  - refinery: Database-agnostic migration runner for Rust (Flyway-like). Great if you mainly need versioned migrations.
  - diesel_migrations: Migrations support for Diesel. Best if you already use Diesel for your ORM layer.
  - sqlparser-rs: Robust SQL parser for many dialects; useful if you want to parse/inspect SQL, but it doesn’t generate or apply schemas by itself.
  
  If your goal is close to “feed a domain model and get vendor-specific SQL plus an installer,” you’ll likely combine a couple of tools (e.g., a model + SeaQuery/SeaORM/diesel for generation/migrations) or use this project’s components.

## Using schema-installer for Migrations

The `schema-installer` crate provides a Flyway-style migration system for applying versioned SQL scripts to your database.

### Quick Start

1. **Create a migrations directory** with SQL files:
```
migrations/
├── V1__create_users.sql
├── V1.1__add_email_index.sql
└── V2__create_posts.sql
```

File naming format: `V{version}__{description}.sql`
- Versions support semantic numbering (1, 1.1, 2.0, 2.0.1, etc.)
- Description becomes a human-readable label (underscores become spaces)

2. **Run migrations:**
```bash
cargo run -p schema-installer -- \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  migrate --migrations-dir ./migrations
```

3. **Check migration status:**
```bash
cargo run -p schema-installer -- \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  info --migrations-dir ./migrations
```

### Commands

#### `migrate` — Apply pending migrations
Applies all migrations not yet recorded in the database. Includes checksum verification to detect if previously-applied migrations have been modified.

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  migrate --migrations-dir ./migrations
```

Output:
```
Applied migration: 1 - create users
Applied migration: 1.1 - add email index
Applied migration: 2 - create posts
```

#### `info` — Display migration status
Shows all migrations (applied and pending) with execution times and checksums.

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  info --migrations-dir ./migrations
```

Output:
```
Version    Description                    Status     Installed At                   Execution (ms)
-----------------------------------------------------------------------------------------------
1          create users                   success    2026-06-23 11:45:47            125
1.1        add email index                success    2026-06-23 11:45:48            42
2          create posts                   pending    -                              -
```

#### `validate` — Verify migration integrity
Checks that all applied migrations haven't been modified (by comparing stored SHA-256 checksums). Useful for detecting accidental or malicious changes to migration files.

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  validate --migrations-dir ./migrations
```

#### `repair` — Fix broken migration state
- Deletes all failed migrations, allowing them to be retried on the next `migrate` run
- Updates checksums for successful migrations (use after intentional edits)

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  repair --migrations-dir ./migrations
```

#### `install` — Legacy XML schema installation
Applies a schema from an XML definition file (for backward compatibility). Records as a single migration.

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  install --schema-file schema.xml
```

### Global Options

Available for all commands:

- `--database-type` (required): `postgresql`, `sqlite`, or `sqlserver`
- `--connection-string` (required): Database connection URL
- `--boolean-mode`: How to represent booleans (`native`, `yesno`, `yn`) — default: `native`
- `--foreign-key-mode`: How to handle relations (`none`, `relations`, `triggers`) — default: `relations`

### Migration Tracking

All migrations are recorded in the `schema_migration` table:

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

### Behavior

- **Versions are ordered numerically**: 1, 1.1, 1.10, 2.0, 10.0 (not lexicographically)
- **Skips already-applied migrations**: Only runs migrations with versions not yet in the database
- **Checksum validation**: Before applying new migrations, re-validates all previously-applied migrations to ensure they haven't been modified
- **Fails fast on checksum mismatch**: Aborts if any applied migration has been altered (prevents applying unverified changes)
- **Execution times tracked**: Records how long each migration took, useful for performance monitoring
- **Supports SQL Server `GO` delimiter**: Automatically uses appropriate statement separators per database type

### Example Workflow

```bash
# 1. Create first migration
cat > migrations/V1__init.sql << 'EOF'
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(100) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL
);
EOF

# 2. Apply it
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://localhost/mydb" \
  migrate --migrations-dir ./migrations

# 3. Check status
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://localhost/mydb" \
  info --migrations-dir ./migrations

# 4. Add second migration
cat > migrations/V2__add_timestamps.sql << 'EOF'
ALTER TABLE users ADD COLUMN created_at TIMESTAMP DEFAULT NOW();
ALTER TABLE users ADD COLUMN updated_at TIMESTAMP DEFAULT NOW();
EOF

# 5. Apply it
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://localhost/mydb" \
  migrate --migrations-dir ./migrations

# 6. Validate all migrations (no one modified them)
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://localhost/mydb" \
  validate --migrations-dir ./migrations
```

### Database Support

| Database   | Supported | Notes |
|-----------|-----------|-------|
| PostgreSQL | ✅ | Full support, uses `$1/$2` placeholders |
| SQLite     | ✅ | Full support, uses `?` placeholders |
| SQL Server | ✅ | Full support, uses `GO` statement separator |

## Workspace crates

- schema-model: core data structures (tables, columns, relations, views, functions, procedures, triggers, other SQL, versions, etc.).
- schema-parser: parsing/processing helpers (WIP).
- schema-sql-generator: backend(s) to create SQL from the model (WIP).
- schema-installer: utilities for applying schemas/migrations (WIP).

## License

Apache-2.0 or MIT, at your option.
