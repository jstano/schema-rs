# schema-migration-generator

Converts a `schema-diff::ChangeSet` into database-specific migration SQL statements (Flyway-style), ready to be applied via `schema-installer`.

## Usage

```rust
use schema_diff::SchemaDiffEngine;
use schema_migration_generator::create_generator;
use schema_model::model::types::DatabaseType;

// Diff two schemas
let change_set = SchemaDiffEngine::diff(&old_schema, &new_schema);

// Generate database-specific migration SQL
let generator = create_generator(DatabaseType::Postgresql);
let mut output = Vec::new();
generator.generate(&change_set, &mut output)?;

let migration_sql = String::from_utf8(output)?;
println!("{}", migration_sql);
```

## CLI Usage

```bash
# Diff two schema sources (file paths, or git '<rev>:<path>' references) and print SQL to stdout
schema-migration-generator \
  --database-type postgresql --old HEAD:schema.xml --new schema.xml

# Shortcut: diff a file on disk against its own HEAD version
schema-migration-generator \
  --database-type postgresql --file schema.xml

# Write to a specific path instead of stdout
schema-migration-generator \
  --database-type postgresql --file schema.xml --output-file migrations/V1__create_users.sql

# Auto-name the output as V{timestamp}.sql alongside the schema file (requires --file)
schema-migration-generator \
  --database-type postgresql --file schema.xml --auto-generate-name

# Auto-name with a description: V{timestamp}__add_users_table.sql
schema-migration-generator \
  --database-type postgresql --file schema.xml --auto-generate-name --description "add users table"
```

`--auto-generate-name` and `--description` require `--file` (not `--old`/`--new`, since there's no single unambiguous schema-file directory to write into otherwise) and are mutually exclusive with `--output-file`. The generated filename is Flyway-style and compatible with `schema-installer`'s migration parser, whether or not a description is given.

## Typical Pipeline

1. **Parse** two schema versions: `schema-parser` + `schema-model`
2. **Diff** them: `schema-diff::SchemaDiffEngine::diff()`
3. **Generate** migration SQL: `schema_migration_generator::create_generator()` + `MigrationGenerator::generate()`
4. **Apply** the SQL: `schema-installer::Migrator::migrate()`

## Database Support

Supported databases with implementations:

| Database   | Status |
|-----------|--------|
| PostgreSQL | ✅ Full |
| SQLite     | ✅ Full |
| SQL Server | ✅ Full |

## Generated SQL Examples

### PostgreSQL

```sql
-- Add table
CREATE TABLE orders (...);

-- Add column
ALTER TABLE users ADD COLUMN created_at TIMESTAMP DEFAULT NOW();

-- Modify column
ALTER TABLE users ALTER COLUMN email SET NOT NULL;

-- Add constraint
ALTER TABLE orders ADD CONSTRAINT fk_orders_user_id 
  FOREIGN KEY (user_id) REFERENCES users(id);

-- Drop column (with rename suggestion for ambiguous case)
-- TODO: possible rename? Column status_id disappeared; created_status present.
--   ALTER TABLE orders RENAME COLUMN status_id TO created_status;
ALTER TABLE orders DROP COLUMN status_id;
```

## Library API

```rust
pub trait MigrationGenerator {
    fn generate(&self, change_set: &ChangeSet, writer: &mut dyn Write) 
        -> Result<(), MigrationGeneratorError>;
}

pub fn create_generator(db_type: DatabaseType) -> Box<dyn MigrationGenerator> { ... }
```

## Ambiguous Rename Handling

When a column is dropped and another of the same type is added, the generator emits a commented-out suggestion before the actual `DROP COLUMN`, helping developers recognize and fix the generated SQL:

```sql
-- TODO: possible rename? Column old_name disappeared; new_name present.
--   ALTER TABLE users RENAME COLUMN old_name TO new_name;
ALTER TABLE users DROP COLUMN old_name;
```

## Part of schema-rs

See the [workspace README](../README.md) for an overview of the full schema-rs toolchain.
