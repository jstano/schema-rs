# schema-rs

A Rust workspace for managing relational database schemas (PostgreSQL, SQL Server, SQLite). A schema is described once in an XML file and the tools turn it into whatever you need:

- **schema-model**: vendor-agnostic schema model.
- **schema-parser**: parses the XML schema into the model.
- **schema-sql-generator**: generates vendor-specific DDL from a schema.
- **schema-diagram-generator**: generates Mermaid or PlantUML ER diagrams from a schema.
- **schema-diff** / **schema-migration-generator**: diff two versions of a schema and generate the SQL migration between them.
- **schema-installer**: installs a schema and applies Flyway-style versioned and repeatable migrations.
- **schema-reverse-engineer**: reverse-engineers a live PostgreSQL or SQL Server database back into an XML schema.

```
XML schema -> [schema-parser] -> model -> [schema-sql-generator]       -> SQL
                                       -> [schema-diagram-generator]   -> Mermaid / PlantUML
                                       -> [schema-diff] -> [schema-migration-generator] -> V{timestamp}__x.sql
                                       -> [schema-installer]           -> database
live database -> [schema-reverse-engineer] -> XML schema
```

## FAQ

- Q: Is there a Rust crate that implements all of this for me?
  
  A: There isn’t a single crate that does exactly what this repository is aiming for (a compact model + parser + SQL generator + installer following this specific design). However, depending on your needs, the following projects can cover large parts of the problem:
  
  - SeaQuery: A builder-style library to programmatically construct SQL; often used by SeaORM. Useful if you want to generate portable SQL without hand-writing it.
  - SeaORM Migrations: Structured migrations integrated with SeaORM that can generate and run SQL for supported databases.
  - refinery: Database-agnostic migration runner for Rust (Flyway-like). Great if you mainly need versioned migrations.
  - diesel_migrations: Migrations support for Diesel. Best if you already use Diesel for your ORM layer.
  - sqlparser-rs: Robust SQL parser for many dialects; useful if you want to parse/inspect SQL, but it doesn’t generate or apply schemas by itself.
  
  If your goal is close to “feed a domain model and get vendor-specific SQL plus an installer,” you’ll likely combine a couple of tools (e.g., a model + SeaQuery/SeaORM/diesel for generation/migrations) or use this project’s components.

## Generating SQL with schema-sql-generator

`schema-sql-generator` turns an XML schema into DDL for PostgreSQL, SQL Server, or SQLite.

```bash
# Create an empty schema.xml to start from
schema-sql-generator --new-schema --schema-file schema.xml

# Generate SQL (written to schema-postgresql.sql next to the input by default)
schema-sql-generator \
  --database-type postgresql \
  --schema-file schema.xml \
  --foreign-key-mode relations \
  --output-file migration/V1__initial_schema.sql
```

Options:

- `--database-type` (required): `postgresql`, `sqlite`, or `sqlserver`
- `--schema-file` (required): path to the XML schema
- `--output-file`: exact output path (parent directories are created); default `{schema-stem}-{database-type}.sql` next to the schema
- `--foreign-key-mode`: `none`, `relations`, or `triggers`
- `--boolean-mode`: `native`, `yesno`, or `yn` (default: the schema file's `booleanMode` attribute)
- `--output-mode`: `all`, `indexes-only`, or `triggers-only`
- `--postgresql-version`: target PostgreSQL major version (e.g. `17`, `18`); affects the UUID default function and whether `pgcrypto` is emitted
- `--sqlserver-version`: target SQL Server product year (e.g. `2022`, `2025`); affects the `json` column type
- `--no-postgres-extensions`: skip the PostgreSQL `create extension` block (`citext`, `btree_gist`)
- `--extension-check-user`: Postgres role checked for superuser privilege in that block (default `CURRENT_USER`)

## Generating Diagrams with schema-diagram-generator

```bash
schema-diagram-generator --format mermaid --schema-file schema.xml
schema-diagram-generator --format plantuml --schema-file schema.xml --output-file docs/schema.puml
```

`--format` is `mermaid` or `plantuml`. Output defaults to `{schema-stem}.mmd` / `{schema-stem}.puml` next to the schema; `--output-file` sets an exact path.

## Generating Migrations with schema-migration-generator

`schema-migration-generator` diffs two versions of an XML schema (via `schema-diff`) and writes the
SQL migration that turns the old one into the new one. Each side is a file path or a git
`<rev>:<path>` reference.

```bash
# Diff schema.xml on disk against its committed version, writing V{timestamp}__add_users.sql next to it
schema-migration-generator \
  --database-type postgresql \
  --file schema.xml \
  --auto-generate-name --description "add users"

# Diff two explicit sources, printing to stdout
schema-migration-generator \
  --database-type sqlserver \
  --old HEAD~1:schema.xml --new schema.xml
```

Options:

- `--database-type` (required): `postgresql`, `sqlite`, or `sqlserver`
- `--file`: shortcut that diffs a file on disk against its latest committed version (`HEAD`)
- `--old` / `--new`: explicit sources, each a file path or `<rev>:<path>`
- `--output-file`: write the migration to this path (default: stdout)
- `--auto-generate-name`: name the output `V{timestamp}[__{description}].sql`, next to `--file` (or in the current directory with `--old`/`--new`); conflicts with `--output-file`
- `--description`: description used in the auto-generated name
- `--output-dir`: override the directory for the auto-generated name (created if missing)
- `--postgresql-version`, `--sqlserver-version`: same as for `schema-sql-generator`

The diff covers tables, columns, keys and indexes, relations, constraints, enum types, views (dependent views are dropped and recreated in dependency order when one changes), functions, procedures, triggers, initial data, and other SQL.

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

File naming format: `V{version}__{description}.sql` — the `__{description}` part is optional (`V{version}.sql` is also valid)

`--migrations-dir` can be a flat directory (as above) or contain per-database subfolders (`postgresql/`, `sqlite/`, `sqlserver/`); the folder matching `--database-type` is used. Mixing loose `.sql` files with vendor subfolders is an error.

Each migration runs inside a transaction. Scripts must not contain their own `BEGIN`/`COMMIT`/`ROLLBACK` (they are rejected before anything runs). For statements that can't run in a transaction (`CREATE INDEX CONCURRENTLY`, `VACUUM`, `ALTER TYPE ... ADD VALUE`), put a Flyway-style config file next to the script, e.g. `V3__add_index.sql.conf`:

```properties
executeInTransaction=false
```

The script's statements then run in autocommit mode (it may manage its own transactions). This can't be atomic: if a statement fails part-way, earlier statements stay applied and the migration is recorded `failed`.
- Versions support semantic numbering (1, 1.1, 2.0, 2.0.1, etc.)
- Description becomes a human-readable label (underscores become spaces)

### Repeatable migrations (`R__`)

A file named `R__{description}.sql` is a *repeatable* migration. It has no version; it is identified by its name and re-applied whenever its contents change. Use these for objects you want to redefine in place rather than version, such as views, functions and grants (`CREATE OR REPLACE ...`).

```
migrations/
├── V1__create_users.sql
├── V2__create_posts.sql
├── R__01_views.sql
└── R__02_grants.sql
```

- **When they run:** a repeatable runs if it has never been applied, or if its checksum differs from its last successful run. A changed repeatable is not a checksum error; it is simply pending again.
- **Order:** within one `migrate`, all pending versioned migrations run first, then pending repeatables in alphabetical order by name. There is no ordering config, so control order with the filename (`R__01_...`, `R__02_...`).
- **`--target`:** repeatables are skipped when `migrate --target <version>` is given, so a rollout stopped at a version does not run scripts written against a newer schema.
- **Tracking:** repeatables are recorded in their own `schema_repeatable_migration` table (one row per name, updated in place), separate from `schema_migration`.
- **`repair`:** as for versioned migrations, deletes failed rows and accepts an edited script by updating its checksum instead of re-running it.
- **Naming:** `R__.sql`, `R_foo.sql` and `R.sql` are errors, as are duplicate names (compared case-insensitively) and names over 200 characters. `executeInTransaction=false` in a `.conf` file works the same as for versioned migrations. `U__` (undo) files are still skipped with a warning.
- **`info`** lists repeatables after the versioned rows with `R` as the version. A repeatable whose script changed since it last ran shows as `Outdated`. **`validate`** reports a failed one or one whose script was removed, but not a changed one. **`pending-check`** exits 1 for a new or changed repeatable.

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

- `--target <version>`: stop after this version even if later migrations exist (repeatables are skipped)
- `--dry-run <path>`: write the SQL of every pending migration to a file instead of applying or recording anything

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
Shows all migrations (applied and pending) with execution times.

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

#### `pending-check` — Check for pending migrations
Prints whether migrations are pending and exits `0` if there are none, `1` if there are (useful in CI).

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  pending-check --migrations-dir ./migrations
```

#### `install` — Install from an XML schema
Applies a schema from an XML definition file. Records as a single migration.

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  install --schema-file schema.xml
```

`install` does not mark any migration as applied, so a later `migrate` would try to re-apply migrations the XML already reflects. Add `--baseline-version <version> --migrations-dir <dir>` to record every migration up to and including that version as applied without running it (Flyway's `baseline`).

#### `install-or-migrate` — Install if new, then migrate
A single startup command: if the database has no successfully applied migrations, it runs `install` from the XML schema and baselines every migration in `--migrations-dir`; it then runs `migrate`, applying anything newer plus repeatable migrations. A database that already has migration history skips the install. `--target` limits the migrate step.

```bash
schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@localhost/mydb" \
  install-or-migrate --schema-file schema.xml --migrations-dir ./migrations
```

### Global Options

Available for all commands:

- `--database-type` (required): `postgresql`, `sqlite`, or `sqlserver` (`mssql` is accepted as an alias)
- `--connection-string` (required): Database connection URL
- `--boolean-mode`: How to represent booleans (`native`, `yesno`, `yn`) — default: `native`
- `--foreign-key-mode`: How to handle relations (`none`, `relations`, `triggers`) — default: `relations`
- `--trust-server-certificate`: SQL Server only; skip TLS certificate verification (self-signed certs)

### Migration Tracking

Versioned migrations are recorded in the `schema_migration` table (repeatable migrations have their own `schema_repeatable_migration` table, described above):

```sql
CREATE TABLE schema_migration (
    id BIGSERIAL PRIMARY KEY,
    version TEXT NOT NULL,
    script_path TEXT NOT NULL,
    checksum TEXT NOT NULL,              -- SHA-256 hash
    execution_time_ms INT NOT NULL,      -- Duration in milliseconds
    installed_at TIMESTAMPTZ DEFAULT now(),
    status TEXT NOT NULL,                -- success, failed
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

## Using schema-reverse-engineer

The `schema-reverse-engineer` crate connects to a live PostgreSQL or SQL Server database and generates a
schema-rs XML schema definition from it — the inverse of `schema-installer`'s `install` command.

### Quick Start

```bash
cargo run -p schema-reverse-engineer -- \
  --connection-string "postgres://user:pass@localhost/mydb" \
  --db-schema public \
  --file schema.xml
```

This introspects the `public` schema (tables, columns, primary/unique/index keys, foreign keys,
check/exclusion constraints, native enum types, views, functions, and procedures) and writes the
result to `schema.xml`.

For SQL Server:

```bash
cargo run -p schema-reverse-engineer -- \
  --database-type sqlserver \
  --connection-string "Server=tcp:localhost,1433;Database=mydb;User Id=sa;Password=...;" \
  --trust-server-certificate \
  --file schema.xml
```

The generated file can be fed straight into `schema-sql-generator` or `schema-installer`.

### Options

- `--database-type`: `postgresql` (default) or `sqlserver`
- `--connection-string` (required): PostgreSQL connection URL, e.g.
  `postgres://user:pass@host:5432/dbname`, or a SQL Server ADO string
- `--db-schema`: schema to introspect — default: `public` (PostgreSQL) / `dbo` (SQL Server)
- `--trust-server-certificate`: SQL Server only; skip TLS certificate verification
- `--file` (required): Path to write the generated XML schema file to

### Notes

- PostgreSQL and SQL Server only — SQLite is not supported.
- Partial and covering indexes and cross-schema foreign keys are kept. Items the schema model can't represent are skipped with a warning.
- SQL Server: `identity` seed/increment other than (1,1), non-persisted computed columns, and
  columnstore/XML/spatial indexes are not carried over, and neither is clustered-ness of keys (the
  generator's defaults apply: clustered primary key, nonclustered everything else). A warning is printed for
  each loss, including a nonclustered primary key or a clustered non-primary index. Table triggers are exported
  verbatim as bottom-ordered `otherSql` (the typed `<triggers>` model can't hold a hand-written trigger).
  PostgreSQL triggers aren't exported.
- SQL Server type mappings: `varchar(max)`/`nvarchar(max)`, `text` and `ntext` become `text`; `money` becomes
  `decimal(19,4)` and `smallmoney` `decimal(10,4)`; `rowversion`, `timestamp` and `image` become
  `binary`; `datetime` and `smalldatetime` both become `datetime` (each of these with a warning). Types with no mapping (`xml`, `geography`,
  `sql_variant`, ...) fall back to `text` with a warning.
- Foreign keys use `ON DELETE` behavior to determine the relation type (`cascade`, `enforce`,
  `setnull`, `donothing`).
- If a table has no primary key but exactly one unique key, that unique key is promoted to the
  primary key.
- `uuid`, `jsonb`/`json`, arrays, and native Postgres enum types are all mapped to their
  corresponding schema-model column types.

## Downloading Prebuilt Binaries (macOS)

The macOS release binaries aren't code-signed/notarized, so Gatekeeper will quarantine
them after download and refuse to run them ("cannot be opened because the developer
cannot be verified"). After unzipping the release archive, clear the quarantine
attribute before running:

```bash
xattr -dr com.apple.quarantine ./schema-installer ./schema-diagram-generator ./schema-sql-generator ./schema-reverse-engineer ./schema-migration-generator
```

(or run it on the whole extracted folder: `xattr -dr com.apple.quarantine ./schema-<version>/`)

## Docker

A Docker image bundling all 5 binaries (`schema-installer`, `schema-diagram-generator`,
`schema-sql-generator`, `schema-reverse-engineer`, `schema-migration-generator`; `git` is included
so `schema-migration-generator` can resolve `<rev>:<path>` sources) is published to Docker Hub as
[`jstano/schema-rs`](https://hub.docker.com/r/jstano/schema-rs), tagged with both the
release version (e.g. `0.4.0`) and `latest`, for `linux/amd64` and `linux/arm64`.

Run any of the bundled tools with `docker run`, passing the tool name as the first argument.
The container is isolated from your host filesystem, so any schema/migration files it needs
to read or write must be shared in with a bind mount (`-v host/path:container/path`):

```bash
docker run --rm jstano/schema-rs schema-installer --help

docker run --rm -v $(pwd)/migrations:/migrations jstano/schema-rs schema-installer \
  --database-type postgresql \
  --connection-string "postgres://user:pass@host:5432/mydb" \
  migrate --migrations-dir /migrations

docker run --rm -v $(pwd)/schemas:/data jstano/schema-rs schema-sql-generator \
  --database-type postgresql \
  --schema-file /data/schema.xml \
  --foreign-key-mode relations \
  --boolean-mode native
```

The generator writes its output next to the input file (`{schema-stem}-{database-type}.sql`),
so with the mount above the resulting `schema-postgresql.sql` appears back in `./schemas` on
the host.

To build and publish the image yourself (requires `docker buildx` and a prior `docker login`):

```bash
just docker-publish
```

## Workspace crates

- schema-model: core data structures (tables, columns, relations, views, functions, procedures, triggers, enum types, other SQL, etc.).
- schema-parser: parses and validates the XML schema into the model; the XSD is published separately as [`schema-xsd`](https://crates.io/crates/schema-xsd) and re-exported as `schema_parser::SCHEMA_XSD`.
- schema-sql-generator: generates PostgreSQL, SQL Server, and SQLite DDL from the model.
- schema-diagram-generator: generates Mermaid and PlantUML ER diagrams from the model.
- schema-diff: computes the set of changes between two schema models.
- schema-migration-generator: turns a schema diff into a vendor-specific SQL migration script.
- schema-installer: installs schemas and applies Flyway-style versioned and repeatable migrations.
- schema-reverse-engineer: reverse-engineers a live PostgreSQL or SQL Server database into a schema-rs XML schema definition.

## License

Apache-2.0 or MIT, at your option.
