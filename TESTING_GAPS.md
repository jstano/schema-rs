# Testing Gap Checklist — schema-rs

## Context
This checklist tracks the biggest testing gaps identified across the workspace, to be worked through incrementally over time. Check items off as they're completed; this file is the source of truth for progress across sessions.

## How to work this list
- Tackle sections top to bottom (roughly priority order), but it's fine to jump around.
- Each checked-off item should land as its own small, reviewable change with passing tests — don't batch unrelated items into one commit.
- Re-run `cargo test --workspace` after each item.
- Update this file (check the box) as part of the same work session that completes the item, so progress persists across conversations.

---

## 1. schema-sql-generator (highest priority — largest gap)

### sqlite/ (currently has zero test files)
- [x] `sqlite_column_type_generator.rs` — type mapping tests (incl. pragmatic/limited type support edge cases)
- [x] `sqlite_table_generator.rs` — CREATE TABLE output
- [x] `sqlite_relation_generator.rs` — foreign key output
- [x] `sqlite_column_constraint_generator.rs`
- [x] `sqlite_index_generator.rs`
- [x] `sqlite_view_generator.rs`
- [x] `sqlite_trigger_generator.rs` (confirmed intentional no-op — sqlite has no trigger support)
- [x] `sqlite_other_sql_generator.rs`
- [x] `sqlite_table_constraint_generator.rs`

### postgresql/ (only column-type generator + util tested)
- [x] `postgres_table_generator.rs`
- [x] `postgres_relation_generator.rs`
- [x] `postgres_function_generator.rs`
- [x] `postgres_procedure_generator.rs`
- [x] `postgres_trigger_generator.rs` (found & documented a real bug: internal `find_table()` mis-parses unqualified `to_table_name` as a schema name — see inline test comment; also the trigger name uses the bare table name, not `{table}_update`)
- [x] `postgres_view_generator.rs`
- [x] `postgres_index_generator.rs`
- [x] `postgres_other_sql_generator.rs`
- [x] `postgres_table_constraint_generator.rs`
- [x] `postgres_column_constraint_generator.rs`
- [x] header-emission logic (UUID v7 function, extensions: uuid-ossp/citext/btree_gist, enum type DDL)

### sqlserver/ (only column-type generator tested)
- [x] `sqlserver_table_generator.rs` (incl. lock_escalation footer override)
- [x] `sqlserver_relation_generator.rs`
- [x] `sqlserver_function_generator.rs` (found a real bug: `output_function`'s drop-if-exists override is dead code — `DefaultFunctionGenerator::output_functions()` calls its own inherent `output_function`, not the trait override, so `output_functions()` never emits the guard; only a direct call to `output_function` does)
- [x] `sqlserver_procedure_generator.rs`
- [x] `sqlserver_trigger_generator.rs`
- [x] `sqlserver_view_generator.rs` (incl. `public` → `dbo` schema mapping)
- [x] `sqlserver_index_generator.rs`
- [x] `sqlserver_other_sql_generator.rs`
- [x] `sqlserver_table_constraint_generator.rs`
- [x] `sqlserver_column_constraint_generator.rs` (incl. enum columns kept, unlike postgres which excludes them)
- [x] GO delimiter output — **found it does not exist**: `SqlGeneratorSettings::new()` hardcodes `";"` for every `DatabaseType`; `DatabaseType::statement_separator()` (which does return `"\nGO"` for SqlServer) is dead code, never called anywhere in the generator crate. Current SQL Server output uses `;`, matching the checked-in `schema-parser-test-schema-sqlserver.sql` reference, but contradicting AGENTS.md's documented behavior. Test added at `common/sql_generator_settings.rs` documents this as current (buggy) behavior.

### common/ (shared orchestration, currently untested)
- [x] `sql_generator.rs` — `DefaultSqlGenerator` orchestration order (pipeline order, `ForeignKeyMode`/`OutputMode` branching) using fake collaborators + a call-log
- [x] `sql_writer.rs` — writer/macro behavior (`print`/`println`/`newline`/`printf` and the `sql_print!`/`sql_println!`/`sql_newline!` macros)
- [x] `table_generator.rs`, `relation_generator.rs` default trait behavior (`output_initial_data` filtering by database type, `output_table` step order, long-table-name constraint-name truncation)

### Doc/implementation mismatch (not a test gap, flag separately)
- [ ] Confirm with user whether MySQL and H2 support should exist (AGENTS.md lists them as supported, but `mysql/`/`h2/` dirs don't exist under `schema-sql-generator/src`) — either implement, or correct AGENTS.md

---

## 2. schema-installer (second priority — core logic untested)

- [ ] End-to-end `migrate` test against sqlite (in-process, easiest target) — apply a set of migrations, verify `schema_migration` table state
- [ ] End-to-end `info` test — verify status reporting for applied/pending migrations
- [ ] End-to-end `validate` test — checksum mismatch detection
- [ ] End-to-end `repair` test — repairing a corrupted migration history
- [ ] Unit tests for `migrator.rs` internals (currently zero)
- [ ] Unit tests for `installer.rs` (currently zero)
- [ ] Unit tests for `connection.rs` `schema_migration` table methods (currently zero)
- [ ] Extend integration coverage to postgres/sqlserver if test infra (e.g. testcontainers) is available; otherwise document sqlite-only as intentional scope

(`sql_split.rs` is already well covered — no action needed.)

---

## 3. schema-model (happy-path only)

- [ ] Re-enable and fix the dead test in `model/schema.rs`: `build_reverse_relations_creates_back_refs`
- [ ] `builder/table.rs` — invalid build cases (duplicate columns, duplicate keys)
- [ ] `builder/column.rs` — invalid build cases
- [ ] `builder/key.rs` — invalid build cases
- [ ] `builder/schema.rs` — multi-schema table lookup edge cases (case-insensitive lookups, missing schema fallback)
- [ ] `enum_type.rs` — invalid/duplicate enum value handling

---

## 4. schema-parser (happy-path only)

- [ ] Test `parse_database_xml` error branch with malformed XML
- [ ] Test missing required attributes/fields
- [ ] Wire up unused fixtures `schema-parser-example-schema.xml` and `schema-parser-empty-schema.xml` into tests (or remove if truly dead)
- [ ] Direct enum parsing tests (not just indirect via table structure)

---

## Verification
For each item: write the test, run `cargo test -p <crate>` (or `--workspace` for cross-cutting items) to confirm it passes, and run `cargo clippy --workspace` to catch anything the new test surfaces. Do not commit without explicit user approval per AGENTS.md commit policy.
