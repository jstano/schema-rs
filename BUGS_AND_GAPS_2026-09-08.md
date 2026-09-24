# Codebase Bug & Gap Analysis — schema-rs (round 2)

Whole-workspace audit, generated 2026-09-08. Follows on from `BUGS_AND_GAPS.md`
(2026-08-21), which closed 32 of 33 items. The one item left open was the one that
mattered most:

> *No `.sql`-execution-based tests anywhere in `schema-sql-generator` — everything is
> asserted as generated-string equality, which is how #2, #6, and #16 shipped unnoticed.*

This round took that gap seriously and **ran** the tools instead of only reading them.
`cargo test --workspace` is green (326 tests) and `cargo clippy` reports only lints — yet
almost nothing the tools produce actually executes.

**Method.** Generated SQL for all three dialects from the repo's own fixture, executed the
SQLite output against real `sqlite3`, drove `schema-installer` through full migration
lifecycles against real SQLite databases, generated both diagram formats, probed edge-case
schemas, and deep-read the installer, the four newer crates, and the model/parser/generator
internals. Findings marked **[v]** were reproduced by running a command; the rest are
code-read with the offending line quoted.

**Headline.** Three areas are not merely buggy but non-functional end to end:

- the **SQLite** generator (output does not parse),
- the **SQL Server** path of `schema-installer` (dies on its first statement),
- **`schema-diff` → `schema-migration-generator`** (cannot migrate a new table at all).

The common cause is structural, not incidental: every test asserts a hand-built input
against a generated *string*, and no test executes a generated artifact or runs a pipeline
end to end. Several tests assert the broken output as expected. Fixing that (Step 0 of the
remediation plan) is worth more than any individual fix below.

---

## Critical — always-broken output on realistic input

- [ ] **C1. Generated SQLite DDL does not execute.** [v]
  ```
  $ sqlite3 t.db < schema-parser-test-schema-sqlite.sql
  Parse error near line 14: near "default": syntax error
  Runtime error near line 92: NOT NULL constraint failed: ParentTable.ID (19)
  Parse error near line 129: unknown database test
  exit=1
  ```
  Four independent defects — C2, C3, C4 and H1 — none visible to the string-equality tests.

  **Status (2026-09-08): partially fixed.** C2, C3, C4 and H1 are all fixed and verified via
  real `sqlite3` execution - the `auto_increment`, missing-FK, unwrapped-`check`, and
  `unknown database test` failures are gone. One *different*, previously-uncatalogued failure
  remains and still makes the fixture fail end-to-end:
  ```
  Parse error near line 14: near "default": syntax error
  char char(1) constraint char default default 'A',
  ```
  The XML fixture's `char` column has `default="default 'A'"` - the literal keyword `default`
  is baked into the attribute value itself, and the generator prepends its own `default `
  unconditionally, so it comes out doubled. This isn't C3/C4/C5/H1/H2 - it's either a fixture
  data mistake (should be `default="'A'"`) or a real gap symmetric to C3's `wrap_in_check`
  fix (skip the generator's own `default ` prefix when the value already starts with it).
  Not yet triaged or fixed.

- [x] **C2. SQLite sequence columns emit MySQL `auto_increment`.** [v]
  `sqlite/sqlite_column_type_generator.rs:23-29` — `fn sequence_sql() { "integer auto_increment" }`.
  SQLite has no `auto_increment`; it parses this as the *column type name*
  `"integer auto_increment"`, so the column has no identity behaviour and every insert
  omitting the PK fails with `NOT NULL constraint failed: ParentTable.ID`. AGENTS.md
  documents the intended mapping as `INTEGER PRIMARY KEY AUTOINCREMENT`. The unit test at
  `sqlite_table_generator.rs:92` asserts `"id integer auto_increment"` — it locks the broken
  output in. (`schema-migration-generator/src/sqlite/mod.rs:20` gets this *right*, so the
  two crates disagree.)

- [x] **C3. User `<check>` constraints are emitted without the `check(...)` wrapper — all dialects.** [v]
  `common/column_constraint_generator.rs:46-58`
  ```rust
  } else if let Some(constraint) = column.check_constraint() {
      Some(constraint.to_string())      // the boolean / enum / min-max branches DO wrap
  ```
  `<check>varcharWithCheck = 'ABC123'</check>` renders everywhere as
  `constraint ck_columntes_varcharwi_353F3BCB varcharWithCheck = 'ABC123'` — a syntax error
  in PostgreSQL, SQL Server and SQLite alike. The checked-in reference `.sql` contains this
  broken form as "expected".

- [x] **C4. Multi-schema models never emit `CREATE SCHEMA`.** [v]
  `grep -ri "create schema" schema-*/src/` → no matches anywhere in the workspace.
  The repo's own fixture declares a `test` schema; all three dialects emit
  `create table test.Unit` with nothing creating `test`. SQLite:
  `Parse error: unknown database test` (SQLite has no schemas — `app.` means an ATTACHed
  database, `types.rs:33-43`). PostgreSQL / SQL Server: `schema "test" does not exist`.

  **Fixed 2026-09-08.** Postgres: `create schema if not exists`. SQL Server: guarded
  `exec('create schema ...')` dynamic SQL (`CREATE SCHEMA` must be the first statement in its
  batch). SQLite: flattens `schema.table` → `schema_table` in `DatabaseType::qualified_name`
  (user's explicit choice - SQLite has no schema concept at all). Also found and fixed a
  third instance of H1's static-dispatch trap while wiring this up:
  `SqlServerGenerator`/`SqliteGenerator::generate()`/`output_sql()` delegated to the inner
  concrete `DefaultSqlGenerator` field, silently bypassing the new `output_header` override.

- [ ] **C5. No identifier quoting anywhere — reserved words produce invalid DDL.** [v]

  **Status (2026-09-08): skipped at user's request**, after surfacing a real design
  conflict: the repo's own fixture has columns named after their own SQL types (`int`,
  `char`, `date`, `text`, `binary`...) to test type mapping, which collides with SQL
  Server's reserved/ODBC keyword list - exactly the case this bug reports as broken.
  User's stated direction is to error out on a bad identifier rather than auto-quote it
  (unlike this doc's own suggested fix), which additionally requires deciding whether to
  validate against a single dialect or the union of all three, and reconciling that
  against the fixture's own type-named columns. Not resolved; revisit later.
  `common/column_generator.rs:140-152`, `key_generator.rs:80-95`, `index_generator.rs:113-131`,
  `relation_generator.rs:71-82` — all `format!("{} {}", column.name(), ...)` with raw names.
  BUGS #16 escaped string *literals* only. A minimal schema with a table named `order` and a
  column named `select`:
  ```
  $ sqlite3 < edge-sqlite.sql
  Parse error near line 1: near "order": syntax error
  ```
  PostgreSQL emits `create table public.order (... select text ...)` — same failure. The
  repo's own fixture already has columns named `int`, `char`, `binary`, `time`, `date`,
  `text`, `json`, `uuid`, and the checked-in reference SQL contains `int integer,` and
  `char nchar(1)`, which SQL Server rejects (`Incorrect syntax near the keyword 'int'`).

- [x] **C6. SQL Server installer is unusable from its first statement.**
  `schema-installer/src/tracking.rs:27,34`
  ```sql
  version NVARCHAR(MAX) NOT NULL,
  CONSTRAINT UQ_schema_migration_version UNIQUE (version)
  ```
  SQL Server forbids LOB types as index key columns → *Msg 1919*. Every SQL Server `migrate`,
  `info`, `validate` and `install` dies at `ensure_migration_table`.
  Immediately behind it, `connection.rs:236` reads the `DATETIME2` column as
  `chrono::DateTime<Utc>`. Verified in the vendored `tiberius-0.11.8/src/tds/time/chrono.rs:75`:
  `FromSql for DateTime<Utc>` matches **only** `DateTimeOffset`, and `Row::get` is
  `try_get(idx).unwrap()` (`row.rs:388`). So as soon as `schema_migration` has one row, SQL
  Server commands **panic** mid-run, leaving a `pending` row behind (→ H9's wedge). Neither
  was caught because `tests/sqlserver_integration_tests.rs` is entirely `#[ignore]`d.

  **Fixed 2026-09-08.** `version`/`script_path`/`checksum`/`status`/`tool_version` are now
  bounded `NVARCHAR(n)`; `installed_at` is read as `chrono::NaiveDateTime` (which `DATETIME2`
  actually maps to) and treated as UTC, with the column's default and the stale-pending
  comparison switched from `GETDATE()` to `SYSUTCDATETIME()` to match. Verified the
  tiberius `FromSql` mismatch directly against the vendored crate source. Could **not** get
  real end-to-end confirmation in this sandbox: SQL Server's Linux container crashes under
  QEMU emulation on this ARM64 host (a pre-existing, already-documented limitation in the
  test file itself, not caused by this fix) - `sqlserver_integration_tests.rs` is still
  `#[ignore]`d and worth running on a native x86_64 host/CI runner to close the loop.

- [ ] **C7. `schema-diff` → `schema-migration-generator` cannot migrate a new table.**

  **Status (2026-09-08): skipped at user's request** ("let's skip this for now"). Not started.
  `schema-diff/src/change.rs:9-11` — `AddTable { table_name }` carries no `Table` payload, and
  `diff_add_columns`/`diff_add_keys`/`diff_add_relations` (`diff_engine.rs:80,138,180,215`)
  all skip tables that don't exist in **both** schemas. So the generators have nothing:
  - `postgresql/mod.rs:20` → `CREATE TABLE {} ();`
  - `sqlite/mod.rs:20` → `CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT);`
  - `sqlserver/mod.rs:20` → `CREATE TABLE {} ();` — a hard T-SQL syntax error

  Adding `orders(id, customer_id FK, amount decimal)` produces literally
  `CREATE TABLE orders ();` — plus, on SQLite, a fabricated `id` column in no model.
  Renames are **structurally unreachable**: `SchemaChange::RenameTable`/`RenameColumn` are
  never constructed by the diff engine (grep-verified), so every rename is emitted as
  `DROP TABLE` + `CREATE TABLE x ()` — total data loss on unchanged data.

---

## High

- [x] **H1. SQLite foreign keys silently vanish — dead trait override.** [v]
  `sqlite/sqlite_table_generator.rs:33-51`. The override of `output_table_definition` appends
  `inline_foreign_key_constraints(table)`, but `output_tables` (line 33) delegates to
  `DefaultTableGenerator::output_tables`, whose loop calls `self.output_table` →
  `self.output_table_definition` on the **`DefaultTableGenerator`** — never on
  `SqliteTableGenerator`. The override is unreachable and the SQLite script contains **zero**
  foreign keys under the default `--foreign-key-mode relations`.
  BUGS #2's fix is therefore dead code: SQLite FKs went from "invalid ALTER syntax" to
  "silently absent". This is BUGS #6's static-dispatch pattern recurring;
  `SqlServerTableGenerator` is the only one that re-implements `output_tables` with
  self-dispatch. Both `PostgresTableGenerator::output_table` and
  `SqliteTableGenerator::output_table` also call *only* `output_table_header` — harmless
  today only because they are unreachable.

  **Fixed 2026-09-08** alongside C2 (same file/code path). `SqliteTableGenerator` and
  `PostgresTableGenerator` now self-dispatch `output_tables`/`output_table`, matching
  `SqlServerTableGenerator`'s existing pattern. Found this exact trap **two more times**
  while fixing it - `SqliteColumnGenerator::column_definitions` delegated to
  `DefaultColumnGenerator`'s (bypassing the new inline-autoincrement column override), and
  `SqlServerGenerator`/`SqliteGenerator::generate()`/`output_sql()` delegated to the inner
  `DefaultSqlGenerator` field (bypassing C4's `output_header` override) - both fixed too.
  See M10 - this pattern has now recurred four times total across this codebase.

- [x] **H2. Boolean defaults are silently coerced to `false`.** [v]
  `common/column_generator.rs:72-83` — `matches!(dc.to_ascii_lowercase().as_str(), "true")`.
  `<column name="active" type="boolean" default="1"/>` → `active boolean default false`.
  Same for `default="yes"`, `default="TRUE "` (trailing space), or a function default. A
  silent data-semantics inversion with no error. Line 82 also gives every boolean column an
  implicit `default false`, so a nullable boolean with no default is inexpressible.

  **Fixed 2026-09-08.** Confirmed via the schema-xsd that `default` is untyped `xsd:string`
  for every column type, so the generator is the only validation layer. Added
  `schema_model::model::column::parse_boolean_default` (recognizes `true`/`1`/`yes`/`on`,
  `false`/`0`/`no`/`off`, trimmed/case-insensitive, plus the existing `null` sentinel for "no
  default"); `Schema::validate()` now rejects anything else with a clear message before
  generation runs (verified live: exits 1 with `ERROR: widget.active has default 'maybe', ...`),
  matching the user's stated preference to error out rather than silently coerce. Omitting
  `default` entirely now correctly emits no default constraint instead of forcing `false`.

- [x] **H3. `char`/`varchar` with no length render `char(0)` / `nvarchar(0)`.** [v]
  `common/column_type_generator.rs:132-138`, `sqlserver_column_type_generator.rs:82-90`
  ```sql
  code nchar(0),
  name nvarchar(0),
  ```
  PostgreSQL: `length for type char must be at least 1`. The test at
  `postgres_column_type_generator.rs:194` asserts `assert_type(ColumnType::Char, "char(0)")`.
  Same path is reached from H4 (numeric attribute fallback) and from a valueless `<enum>`
  (`column_type_generator.rs:146-165` leaves `max_length = 0`).

  **Fixed 2026-09-08.** Matching the user's stated preference to error rather than silently
  coerce (same direction as H2 and C5), `Schema::validate()` now rejects any `Char`/`Varchar`
  column with `length <= 0` before generation runs, naming the table/column/type. This also
  closes off the H4 path into it, since `attr_i32` (below) no longer silently turns a bad
  `length` into `0`. The generator-level tests that asserted `char(0)`/`varchar(0)` as expected
  output (`postgres_column_type_generator.rs`, `sqlite_column_type_generator.rs`) were updated
  to use a valid length instead of locking in the broken value.

- [x] **H4. Numeric and boolean XML attributes silently fall back instead of erroring.** [v]
  `schema-parser/src/parser/roxml_parser.rs:385-399` — `.parse::<i32>().ok()` /
  `.parse::<f64>().ok()` / `attr_bool`. `length="1O0"` (letter O) or `length="99999999999"`
  (i32 overflow) → `None` → `.length(0)` → H3. `minValue="1o"` → the constraint silently
  vanishes. `required="yep"` → `false`. BUGS #22/#23 made every other typo'd attribute a hard
  error; numerics and booleans are the remaining silent holes. (`maxValue="inf"` also parses,
  giving `check(x <= inf)`.)

  **Fixed 2026-09-08.** `attr_bool`/`attr_i32`/`attr_f64` in `roxml_parser.rs` now return
  `Result<Option<T>, String>` instead of `Option<T>`: a *missing* attribute still yields `Ok(None)`,
  but a *present, unparseable* one (bad boolean spelling, non-numeric int/float, i32 overflow)
  is a hard error naming the element, attribute, and offending value, propagated with `?` from
  every call site (all of which already returned `Result`). `attr_f64` also now rejects
  non-finite values (`inf`/`nan`), closing the `maxValue="inf"` gap called out above. Verified
  with new unit tests for each function's error path, alongside the existing case-insensitivity
  test (updated for the new `Result` return type).

- [x] **H5. A typo'd `booleanMode`/`foreignKeyMode` panics instead of returning `Err`.** [v]
  `schema-parser/src/parser/convert.rs:35,41` — `.unwrap()` on the parse `Result`.
  ```
  $ schema-sql-generator --schema-file bad.xml ...
  thread 'main' panicked at schema-parser/src/parser/convert.rs:35:10:
  called `Result::unwrap()` on an `Err` value: "Unknown boolean mode: yes_no"
  ```
  `parse_database_xml` declares `Result<DatabaseModel, String>`. These two root-element
  attributes were missed by the BUGS #4/#12/#23 sweep.

  **Fixed 2026-09-08.** Replaced both `.unwrap()` calls with `?` — `BooleanMode`/`ForeignKeyMode`'s
  `FromStr::Err` is already `String`, matching `convert_database`'s own return type, so no
  extra mapping was needed. A typo'd mode now returns `Err("Unknown boolean mode: ...")` /
  `Err("Unknown foreign key mode: ...")` instead of panicking. Added unit tests covering the
  invalid-value error path for both attributes, the valid-value path, and the missing-attribute
  default (`Native`/`Relations`).

- [x] **H6. An empty constraint string emits a stray comma and blank line inside `create table`.** [v]
  `common/column_constraint_generator.rs:28-40` returns `String::new()`, which
  `table_generator.rs:52-72` never filters out. `columns_with_check_constraints` includes a
  Boolean column with `min`/`max` set, but `check_constraint_sql` takes the Boolean branch
  first and returns `None` under `BooleanMode::Native`:
  ```sql
  create table public.order
  (
     ...
     select text,

  );
  ```
  Invalid on all three dialects.

  **Fixed 2026-09-08.** `DefaultTableGenerator::output_table_definition_with_extra`
  (`table_generator.rs`) now filters empty strings out of the combined column/key/constraint
  list before printing, so an empty constraint entry from any generator no longer produces a
  stray trailing comma or blank line inside `create table` — fixed once for all three dialects,
  since every dialect's `output_table_definition` funnels through this shared method. Added a
  regression test (`output_table_definition_drops_empty_constraint_strings`) reproducing the
  exact H6 shape via a stub constraint generator that returns `vec![String::new()]`. Left M3
  (the underlying if/else-if chain that silently drops the min/max check when a Boolean column
  also has one) open — that's a separate, deliberate follow-up, not part of this fix.

- [x] **H7. Two triggers on one table collide; the second silently deletes the first.** [v]
  `postgresql/postgres_trigger_generator.rs:151,154,228,231` — both `drop trigger`/`create
  trigger` use the bare `table_name`:
  ```sql
  create trigger parenttable after delete on public.ParentTable ...
  drop trigger if exists parenttable on public.ParentTable cascade;   -- drops the one above
  create trigger parenttable after insert or update on public.ParentTable ...
  ```
  The backing functions *are* correctly suffixed (`parenttable_delete`/`parenttable_update`);
  only the trigger name is not. Under `--foreign-key-mode triggers` this silently loses
  cascade/set-null delete propagation for every table with both. SQL Server gets it right
  (`sqlserver_trigger_generator.rs:69,169`) — the dialects diverge.

  **Fixed 2026-09-08.** Both `drop trigger`/`create trigger` statements now reuse the
  already-computed `fn_name` (`{table}_delete` / `{table}_update`) instead of the bare
  `table_name`, matching the SQL Server generator's naming. Updated the one unit test that
  asserted the old, colliding trigger name, and regenerated the checked-in PostgreSQL
  reference SQL to confirm `parenttable_delete`/`parenttable_update` now appear as two
  distinct triggers.

- [x] **H8. `generate_uuid()` depends on `pgcrypto`, which is never created.** [v]
  `postgres_generator.rs:50` uses `gen_random_bytes(10)`; `create_extensions` (78-79) creates
  only `citext` and `btree_gist`. `create function` succeeds (plpgsql bodies aren't resolved
  at creation), so this fails at the first insert into a Uuid column:
  `function gen_random_bytes(integer) does not exist`. AGENTS.md also still claims `uuid-ossp`
  is emitted; it is not.

  **Fixed 2026-09-08.** `create_extensions()` now also emits
  `create extension if not exists "pgcrypto"`, gated on the same
  `target_postgres_version() < 18` condition that governs whether
  `create_uuid_generator_function` (the only caller of `gen_random_bytes`) is emitted at all —
  PostgreSQL 18's native `uuidv7()` needs no extension. Added unit tests asserting `pgcrypto`
  is present before version 18 and absent from 18 onward. Also corrected AGENTS.md's stale
  claim that `uuid-ossp` is emitted (it never was) to name the actual extensions.

- [x] **H9. A crash between the migration commit and the status update wedges the database permanently.**
  `schema-installer/src/migrator.rs:181-201`, `connection.rs:104-160`. The `pending` row, the
  migration itself, and the `success` update are **three separate transactions**. Kill the
  process after the DDL commits but before the status update and you get an unrecoverable
  loop: `migrate` → 60 s `LockTimeout` (repeating until the 600 s stale threshold) → `repair`
  deletes the pending row → `migrate` re-runs applied DDL → `already exists` → `failed` →
  `repair` → … Only manual SQL recovers. For PostgreSQL and SQLite the fix is free: write the
  tracking row inside the same transaction as the migration.

  **Fixed 2026-09-08.** Replaced `AnyPool::execute_transactional` with
  `execute_migration_transactional`, which runs the migration's DDL statements *and* the
  `status = 'success'` update as one transaction (all three dialects, not just
  PostgreSQL/SQLite - SQL Server's tiberius path already did its own manual
  `BEGIN`/`COMMIT`, so folding the update in cost nothing extra there either). A crash before
  commit now leaves the tracking row exactly `"pending"` with none of the DDL applied
  (safe to reclaim via the existing stale-pending cleanup); a crash after commit leaves it
  `"success"` with the DDL applied - the two can no longer disagree. Also added the same
  `rows_affected() == 0` guard `update_migration_status` already had (if the tracking row
  vanished - e.g. a racing `repair` - the whole transaction now rolls back instead of
  silently committing DDL nobody recorded). `migrator.rs`'s failure path is unchanged: on a
  statement error the transaction is never committed (rolled back for free) and `"failed"` is
  still recorded via a separate, deliberately non-atomic `update_migration_status` call,
  since a failed attempt has no DDL side effect left to stay atomic with. Added
  `test_sqlite_migration_ddl_and_success_status_commit_atomically`, which drives
  `execute_migration_transactional` directly with a migration id that doesn't exist and
  asserts the DDL was rolled back - proving the DDL and the status write are truly one
  transaction rather than merely sequential calls.

- [x] **H10. `validate` passes on exactly the states `migrate` rejects.** [v]
  `migrator.rs:277-311` walks applied→source only and `continue`s on any non-`success` row.
  Verified against real SQLite databases — `validate` exits 0 and `migrate` exits 1 on the
  identical state, three different ways:
  - a `failed` row (`migrate`: *"…run `repair` before retrying"*);
  - an out-of-order pending migration (`migrate`: `OutOfOrderMigration`);
  - a migration on disk never applied at all (Flyway: *"Detected resolved migration not
    applied to database"*).

  Any CI gate built on `validate` goes green and the deploy then breaks.

  **Fixed 2026-09-08.** `validate` no longer `continue`s past a non-`success` applied row —
  a `failed` or stuck `pending` row is now reported as an issue (same message direction as
  `migrate`'s "run `repair` before retrying"). Added a second check mirroring `migrate`'s
  out-of-order guard: any source migration older than the highest successfully-applied
  version that was never applied is now reported as *"Detected resolved migration not
  applied to database"*, closing the gap where `validate` never looked at unapplied source
  migrations at all. Added `test_sqlite_validate_detects_failed_migration` and
  `test_sqlite_validate_detects_out_of_order_unapplied_migration`, both driven against real
  SQLite databases mirroring the existing `migrate`-side regression tests for the same
  states.

- [x] **H11. One non-`V` file in the migrations directory bricks every command.** [v]
  ```
  $ schema-installer … info --migrations-dir ./m
  Error: InvalidConfiguration("Migration filename must start with V (case-insensitive): R__refresh.sql")
  EXIT=1
  ```
  `migration.rs:106`'s `?` aborts the whole directory scan. A Flyway user dropping in a
  repeatable (`R__`) or undo (`U__`) migration loses `migrate`, `info`, `validate`, `repair`
  **and** `pending-check` — including the one command that would diagnose it.

  **Fixed 2026-09-09.** `parse_migration_filename` now returns `Result<Option<(String,
  String)>, _>`: a filename that doesn't start with `V`/`v` (repeatable `R__`, undo `U__`,
  or any other stray `.sql` file) is skipped with a `Warning: skipping non-versioned
  migration file...` message instead of aborting the scan; `DirectoryMigrationSource::migrations`
  `continue`s past it. A filename that *does* start with `V` but is otherwise malformed still
  hard-errors, since that shape is far more likely to be a typo in a real migration than an
  intentional non-versioned file. Verified end-to-end against a real SQLite directory
  containing `V1__create_users.sql` + `R__refresh_view.sql`: both `info` and `migrate` now
  exit 0, print the warning, and correctly process `V1`. Added
  `test_parse_migration_filename_skips_non_versioned_files`.

- [x] **H12. `repair` is the one command that fails on an un-set-up database.** [v]
  ```
  $ schema-installer … repair --migrations-dir ./m
  Error: Database("… (code: 1) no such table: schema_migration")
  ```
  `migrator.rs:349` goes straight to `delete_failed_migrations()`; every other entry point
  calls `ensure_migration_table` first (`migrator.rs:89,213,269,330`).

  **Fixed 2026-09-09.** `Migrator::repair` now calls `pool.ensure_migration_table(&config.database_type)`
  before `delete_failed_migrations()`, matching `migrate`/`info`/`validate`. Added
  `test_sqlite_repair_succeeds_on_an_un_set_up_database`, which runs `repair` against a
  freshly-created SQLite database with no prior migration activity (no `schema_migration`
  table) and asserts it succeeds instead of erroring. Verified against real SQLite; full
  workspace test suite remains green.

- [x] **H13. Duplicate migration versions — the second is silently never executed.**
  `migration.rs:106-118` has no duplicate-version check (Flyway hard-errors). With
  `V1__add_users.sql` and `V1__add_orders.sql` present, the first wins the slot; the second
  hits the `UNIQUE (version)` violation, is misread as `ConcurrentMigrationDetected`, sees
  `status == "success"` and prints *"…already applied by another process; skipping"*. It is
  never executed and never recorded, and which one wins depends on `read_dir` order.

- [x] **H14. The `install` path is not transactional, and cannot be retried.**
  `installer.rs:149-159` loops `pool.execute_sql(&statement)` one autocommit statement at a
  time, while `migrator.rs:401` uses the ready-made `execute_transactional`. Failing on
  statement 23 of 40 leaves 22 tables behind with nothing rolled back. Re-running then fails:
  `check_if_installed` (`installer.rs:130-142`) is
  `migrations.iter().any(|m| m.status == "success")`, so a leftover version-`0` row collides
  on insert and surfaces as `ConcurrentMigrationDetected("0")` with no concurrency involved —
  and, worse, running **any** migration first makes `install` report *"Schema is already
  installed. Skipping"* and exit 0 without creating a single table.

- [x] **H15. SQL Server drop guards check the wrong schema.** [v]
  `sqlserver_table_generator.rs:64`, `sqlserver_view_generator.rs:34`, `sqlserver_trigger_generator.rs`
  ```sql
  if exists (select name from dbo.sysobjects where name = 'Unit' and type = 'U')
  drop table test.Unit
  ```
  The guard queries `dbo.sysobjects` on **name only**, then drops a **schema-qualified**
  object. `dbo.Unit` exists but `test.Unit` doesn't → the drop errors; `test.Unit` exists but
  `dbo.Unit` doesn't → the drop is skipped and `create table` errors. `sysobjects` is also
  deprecated; `object_id('test.Unit','U')` is the correct form.

- [x] **H16. SQL Server: multiple `identity` columns per table.** [v]
  `sqlserver_column_type_generator.rs:26-32`. The fixture's `ColumnTesterTable` has a
  `Sequence` and a `LongSequence` column →
  ```sql
  sequence integer identity(1,1) not null,
  longsequence bigint identity(1,1),
  ```
  SQL Server permits exactly one → *Msg 2744*. `Table::identity_column()` (`table.rs:152`)
  already assumes there is only one. Identity is also applied whether or not the column is
  part of the PK.

- [x] **H17. Table DROP order is not dependency-ordered.** [v]
  Drops are emitted in declaration order, each immediately before its `create`. PostgreSQL is
  saved by `cascade` (`table_generator.rs:102`); SQL Server and SQLite are not
  (`sqlserver_table_generator.rs:52-70`). Re-running a script against a populated database
  fails as soon as an earlier-dropped table is a FK target of a later one.

- [x] **H18. Table-level `<constraint databaseType="…">` is never filtered by dialect.**
  `common/table_constraint_generator.rs:31-36` — no filter, while views (`schema.rs:99-105`)
  and initial data (`table_generator.rs:132-136`) *are* filtered. A
  `<constraint databaseType="postgresql">check (email ~ '^[^@]+@')</constraint>` is emitted
  verbatim into the SQL Server and SQLite scripts. `Constraint.database_type` is decorative.

- [x] **H19. Diff comparisons ignore the attributes that matter.**
  `schema-diff/src/diff_engine.rs`
  - `relations_equal` (233-238) ignores `relation_type()` → changing a FK from `cascade` to
    `setnull` produces **no change**; production keeps `ON DELETE CASCADE`, so deleting a
    parent still cascades — exactly what the change was meant to stop.
  - `keys_equal` (157-163) ignores `is_unique()`, `is_cluster()`, `include()` → dropping
    `unique="true"` produces no change; an `<index>` and a `<unique>` on the same columns
    generate the same name → `relation already exists`.
  - `columns_differ` (114-121) ignores `enum_type`, `element_type`, `generated`, `min_value`,
    `max_value`.
  - Constraints (195-199) and views (260-264) are compared **by name only** → editing a check
    body or a view's SELECT can never be migrated.

- [x] **H20. Migration DDL names don't match the names the create path produced.** [v]

  | Object | Created by `schema-sql-generator` | Dropped by `schema-migration-generator` |
  |---|---|---|
  | PK | `pk_<table>` (`common/key_generator.rs:4`) | `<table>_pkey` (`postgresql/mod.rs:240`) |
  | FK | `fk_<table><n>` (`common/relation_generator.rs:39`) | `fk_<table>_<column>` (`postgresql/mod.rs:275`) |
  | Index | `ix_<table><n>` (`postgres_index_generator.rs:60`) | `idx_<table>_<cols>` (`postgresql/mod.rs:214`) |

  Dropping a relation emits `ALTER TABLE orders DROP CONSTRAINT fk_orders_customer_id;`
  against a constraint actually named `fk_orders1` → hard failure. The index case is worse
  because `DROP INDEX IF EXISTS` makes it a **silent** no-op: the index survives, the
  migration reports success, and every subsequent diff re-emits the same dead statement.

  Fixed by extracting the naming rules into a new `schema_model::naming` module (single
  source of truth for `pk_`/`ak_`/`ix_`/`fk_` names, char-safe truncation included) and
  having both `schema-sql-generator`'s `common/key_generator.rs`, `relation_generator.rs`,
  `index_generator.rs` (create path) and all three `schema-migration-generator` dialects
  (alter path) call into it, so the two can never drift apart again. Since `ak_`/`ix_`/`fk_`
  names are positional (numbered by the object's position among its siblings on the
  table), `schema_diff::SchemaChange::{AddKey,DropKey,AddRelation,DropRelation}` gained an
  `ordinal` field computed by `diff_engine.rs` from the old/new table's key/index/relation
  list order, so the migration generator can reconstruct the exact same name. Regression
  tests in `schema-migration-generator/src/tests.rs` diff a schema change end-to-end and
  assert the emitted DROP statement names match `schema_model::naming` for PK/FK/unique
  key/index drops.

- [x] **H21. Non-ASCII column names are never found — and get dropped by the diff.**
  `schema-model/src/model/table.rs:128-150`
  ```rust
  let lower = column_name.to_lowercase();                              // Unicode lowering
  self.columns.iter().find(|c| c.name().eq_ignore_ascii_case(&lower))  // ASCII-only compare
  ```
  For `ÉTAT`: `to_lowercase()` → `état`, and `"ÉTAT".eq_ignore_ascii_case("état")` is `false`.
  So `Table::column("ÉTAT")` **panics** on a column that exists, and `schema-diff` emits
  `DropColumn` + `AddColumn` on every run against an unchanged schema → `ALTER TABLE … DROP
  COLUMN "ÉTAT"` → data loss. The `to_lowercase()` call is the bug. Three different
  conventions coexist: `Schema::table_index` (`schema.rs:84-88`) lowercases both sides
  (correct), `Key::contains_column` (`key.rs:86-88`) compares raw (correct), this one is
  neither.

- [x] **H22. Duplicate table names silently overwrite the lookup index.**
  `schema-model/src/model/schema.rs:194-198`
  ```rust
  self.table_map.insert(table.name().to_lowercase(), idx);   // duplicate silently overwrites
  self.tables.push(table);
  ```
  Two `<table name="Users">` → both emitted as `create table Users` (the second fails at
  runtime) and every lookup resolves only the *second*, so relations to the first silently
  target the wrong table. `Schema::validate()` does not check this (BUGS #32 left it open).

- [x] **H23. Reachable panics that `validate()` does not catch.**
  BUGS #27/#32 claimed to close this by validating up front. Still live:
  - `common/column_type_generator.rs:148`, `postgres_column_type_generator.rs:112`,
    `sqlserver_column_type_generator.rs:115` — `column.enum_type().as_ref().unwrap()`.
    `<column type="enum"/>` with no `enumType` passes `validate()` and panics in generation.
  - `postgres_column_type_generator.rs:77-83,94` — panics on an unparseable `elementType` and
    on `other => panic!("Unsupported array element type")`. `validate()` only checks that
    `elementType` is *present*, so `elementType="uuid"` / `boolean` / `date` / `json` panics.
  - `sqlserver_column_type_generator.rs:59`, `sqlite_column_type_generator.rs:52` —
    `panic!("… does not support arrays")`. One schema.xml is meant to target all three
    dialects, so any PostgreSQL array column crashes the SQL Server and SQLite runs.
  - `sqlite_procedure_generator.rs:27,32` — still panics when a `databaseType="sqlite"`
    procedure exists (BUGS #1 only gated the empty case).

- [x] **H24. Reverse engineer discards the schema name and drops whole index sets.**
  - `reader.rs:29` — `SchemaBuilder::new(None::<&str>)`. `--db-schema sales` only filters
    queries; the output XML has no schema name, so regenerating emits `create table
    public.orders`, targeting the wrong schema.
  - `xml_writer.rs:143-161` — `let Some(primary) = table.primary_key() else { return; }`.
    A PK-less table loses **all** unique keys and indexes from the round trip.
  - `postgres/keys.rs:88-112` iterates a `HashMap`, discarding the query's `ORDER BY` → two
    runs against an unchanged database produce differently-ordered XML.
  - `postgres/columns.rs:187` returns `Err(UnsupportedColumnType)`, propagated out of
    `main.rs` — a single `inet`, `interval`, `money`, `xml`, `hstore` or PostGIS column
    anywhere aborts the run with no output file and no skip-with-warning path.

---

## Medium

### SQL generator

- [x] **M1. `include`, `compress`, `cluster` are parsed then thrown away.**
  `common/index_generator.rs:141-143` — `fn index_options(&self, _key) -> Option<String> { None }`,
  and all three dialects delegate. `<index unique="true" include="name,code" compress="true">`
  → `create index ix_t1 on dbo.t (id)`. Worse, `sqlserver_key_generator.rs:12` hard-codes
  `primary key nonclustered`, so `<primary cluster="true">` produces the exact opposite of
  what was asked.
- [ ] **M2. `generated="…"` is silently dropped by every generator.** `Column::generated` is read
  only by `schema-reverse-engineer/src/xml_writer.rs:116`. Reverse-engineer a computed
  column → regenerate → it comes back as a plain column.

  **Status (2026-09-15): investigated, not started.** Confirmed via `grep` that no
  `ColumnGenerator`/`ColumnTypeGenerator` impl (Postgres/SQLite/SQL Server) ever calls
  `column.generated()` - `DefaultColumnGenerator::column_sql`/`column_options`
  (`common/column_generator.rs`) go straight from `column_type_sql()` to `default_value()`,
  so a computed column round-trips as an ordinary column with its default silently absent too
  (the reverse-engineer already suppresses `default_constraint` for `is_generated` columns,
  `postgres/columns.rs:115`).

  Real complication found: today `schema-reverse-engineer/src/postgres/columns.rs:103-107`
  stores the **full Postgres clause** in `generated` - `"generated always as ({}) stored"` -
  not a bare expression. That string happens to also be valid SQLite syntax (SQLite supports
  the identical `GENERATED ALWAYS AS (...) STORED` column-constraint form), but it is not
  valid SQL Server syntax: SQL Server computed columns use `column_name AS (expr) [PERSISTED]`
  with **no data type declared at all** - the current `"{name} {type} {options}"` assembly in
  `column_sql` can't take a raw pre-formatted Postgres clause and make it correct in all three
  dialects simultaneously.

  Direction picked, not yet coded: treat `generated` as a *bare expression* end to end
  (matching how `check`/`default` are already raw, dialect-portable fragments the generator
  wraps) rather than a pre-formatted clause:
  1. `schema-reverse-engineer/src/postgres/columns.rs`: store just
     `row.generation_expression.unwrap_or_default()`, not the wrapped clause. No test asserts
     the current wrapped string (checked - only `xml_writer.rs`/`reader.rs` touch the field),
     so this is safe to change.
  2. Add `ColumnTypeGenerator::generated_column_sql(&self, table: &Table, column: &Column, expr: &str) -> String`
     with a default impl `format!("{} generated always as ({}) stored", self.column_type_sql(table, column), expr)`
     (covers Postgres and SQLite, which share this syntax) and an override on
     `SqlServerColumnTypeGenerator` returning `format!("as ({}) persisted", expr)` (no type
     prefix).
  3. In `DefaultColumnGenerator::column_sql`: when `column.generated()` is `Some(expr)`, branch
     to `"   {name} {generated_column_sql}"` (+`" not null"` when `column.required()`) instead
     of the normal type+options path, and skip `default_value()` entirely - all three dialects
     forbid combining `DEFAULT` with a generated/computed column.
  - `PostgresColumnGenerator`/`SqliteColumnGenerator`/`SqlServerColumnGenerator` all delegate
    `column_sql` straight through to the shared `DefaultColumnGenerator` (verified by reading
    each file), so step 3 alone covers Postgres and SQL Server; `SqliteColumnGenerator` has its
    own `column_sql` override but only short-circuits for the inline-autoincrement-PK case and
    falls through to the shared one otherwise, so it's covered too.
  - Not yet decided: whether SQL Server's generated columns should always be `PERSISTED` (my
    lean, for parity with Postgres/SQLite always being physically stored) or left virtual by
    default - worth confirming before implementing.
- [ ] **M3. Only one check constraint per column can ever be emitted.**
  `column_constraint_generator.rs:46-58` is an if/else-if chain, so a column with both
  `<check>` and `minValue`/`maxValue` silently loses the min/max. The hashed name is per
  (table,column), so emitting both would collide anyway.
- [ ] **M4. PostgreSQL enum types are emitted unqualified.** [v] (`postgres_generator.rs:104-105`)
  while the columns using them are fully qualified. Lands wherever `search_path` points;
  two schemas with a same-named enum → the second `drop type … cascade` removes the first.
- [ ] **M5. Cross-schema enum references are impossible.** `column_type_generator.rs:147-148` →
  `Schema::get_enum_type` (panics), and `validate()` (`schema.rs:169-178`) only looks in the
  column's own schema. A database-level `<enum>` used from inside `<schema name="app">` is
  rejected as *"not defined in this schema"*.
- [ ] **M6. SQL Server type mappings lose semantics.** [v] `Date`/`Time`/`Timestamp` all →
  `datetime` (should be `date`/`time`/`datetime2`); `Json` → `json`, a type that does not
  exist before SQL Server 2025 (should be `nvarchar(max)`).
- [ ] **M7. `<aggregations>` emits empty trigger shells and nothing else.**
  `postgres_trigger_generator.rs:25,39` gates trigger emission on
  `!table.aggregations().is_empty()`, but no aggregation SQL is ever generated —
  `Aggregation::aggregation_columns/groups/frequency` have no non-test call sites.
- [ ] **M8. Truncated identifiers can still exceed PostgreSQL's limit** (BUGS #15 partially
  fixed): `key_generator.rs:34-60` etc. test with `full_name.len()` (**bytes**) but truncate
  with `.chars().take(available)`. PostgreSQL's 63 is a byte limit, so a Cyrillic table name
  truncates to 60 chars = 120 bytes and PostgreSQL silently truncates further — potentially
  colliding with a sibling whose only difference is the cut-off numeric suffix.
- [ ] **M9. No dependency ordering for views/functions.** Views are emitted in declaration order
  and functions come *after* triggers but *before* procedures (`sql_generator.rs:126-130`).
  A view selecting from a later-declared view fails.
- [ ] **M10. Remaining static-dispatch override traps** (the BUGS #6 pattern), currently masked
  only because the overrides are pure delegation: `DefaultIndexGenerator::output_indexes_for_table`
  (so a dialect `index_options` override — the natural place to implement M1 — would never
  run), `OtherSqlGenerator::output_other_sql`, `PostgresFunctionGenerator::output_function`.
- [ ] **M11. `Schema::validate()` gaps BUGS #32 left open, confirmed still open:** duplicate table
  (H22) and column names, key/index columns existing on the table
  (`<primary><column name="Idd"/></primary>` → runtime error), relation *target column*
  existence (only the target *table* is checked, `database_model.rs:133-144`), and the
  `from_column` existence check runs only inside the `RelationType::SetNull` arm
  (`schema.rs:141-165`).
- [ ] **M12. Schema-name lookup is case-sensitive while table/column lookup is not**
  (`database_model.rs:50-60` uses `==`), contradicting the documented model contract.
  `find_table_by_qualified_name` (line 91) also treats any name without exactly two
  dot-separated parts as unqualified, so `db.schema.table` resolves to a table literally
  named `db.schema.table`.

### Installer

- [ ] **M13. `script_path` stores an absolute local path.** [v]
  `/tmp/.../scratchpad/migrations/V1__create_users.sql`. Differs between dev, CI and prod for
  identical migrations, and is meaningless for `EmbeddedMigrationSource`.
- [ ] **M14. `info` shows the filename, not the description, for applied rows.** [v] `migrate`
  prints `1 - create users`, `info` prints `V1__create_users.sql`. There is no `description`
  column on `schema_migration` (Flyway has one).
- [ ] **M15. Nested block comments break statement splitting** (`sql_split.rs:112-119`, and the
  `GO` variant at 283-296) — PostgreSQL guarantees `/* */` nests, so a commented-out
  `DROP TABLE users;` inside a nested comment is **executed**.
- [ ] **M16. Backslash escapes in `E'…'` break quote tracking** (`sql_split.rs:69-80`, which
  handles only doubled `''`) → `E'it\'s; here'` is split mid-literal.
- [ ] **M17. Splitter end-state is never validated** (`sql_split.rs:123-126`) — an unterminated
  `$$` body yields one giant "statement" and an unhelpful error thousands of characters in.
- [ ] **M18. No way to run a migration outside a transaction** (`migrator.rs:398-401`) →
  `CREATE INDEX CONCURRENTLY`, `VACUUM`, `ALTER TYPE … ADD VALUE` can never be migrated.
  Relatedly, a script with its own `BEGIN;`/`COMMIT;` nests, the inner `COMMIT` closes the
  outer transaction, `tx.commit()` fails, and a fully-applied migration is recorded
  `failed` → H9's wedge.
- [ ] **M19. Version parsing and version comparison disagree.** `migration.rs:180` accepts any
  digit run but `:207` does `.parse::<u64>().ok()` → a 20-digit timestamp version (a standard
  Flyway convention) parses to nothing and sorts *before* `V1`, rejecting everything else as
  out-of-order. Leading zeros are worse: `V01` and `V1` compare `Equal` for sorting but are
  distinct strings for the "already applied" check, so **both** are applied, in
  nondeterministic order.
- [ ] **M20. Errors are converted into "everything is pending"** (`migrator.rs:330-334`) —
  `ensure_migration_table(…).is_err()` → `return Ok(true)` and
  `get_applied_migrations().await.unwrap_or_default()`. A permissions failure makes
  `pending-check` report unapplied migrations instead of bad credentials.
- [ ] **M21. TLS is forced on for SQL Server** (`connection.rs:50`) — `config.encryption(Required)`
  runs *after* `from_ado_string`, discarding the user's `Encrypt=false`.
- [ ] **M22. `is_installed`/`get_installed_version` sniff error strings** (`installer.rs:121,135`)
  for `"does not exist"`/`"no such table"`. SQL Server says `Invalid object name` — matching
  neither, so a fresh-database bootstrap returns `Err` instead of `Ok(false)`.

### Diff / migration generator / diagrams / reverse engineer

- [ ] **M23. Migration type mapping diverges from the create-path mapping** → permanent diff
  churn. `postgresql/mod.rs:194,198` map `Varchar → text` and `Enum → text` while the create
  path emits `varchar(n)` and the native enum type. A column added by migration can never
  converge with the same column created fresh; enum columns lose their type entirely.
- [ ] **M24. `ALTER COLUMN TYPE` emits `serial` and never a `USING` clause**
  (`postgresql/mod.rs:67-78,170`) → `type "serial" does not exist`, and `varchar`→`int`
  without `USING` → `cannot be cast automatically`.
- [ ] **M25. NOT NULL columns are added with no default and no backfill** (`postgresql/mod.rs:31`,
  `sqlite/mod.rs:31`, `sqlserver/mod.rs:38`) → `column "status" contains null values`; on
  SQLite it fails even on an empty table.
- [ ] **M26. `AddConstraint` double-wraps the body in `CHECK (…)`** (`postgresql/mod.rs:122`,
  `sqlserver/mod.rs:97`) → `ADD CONSTRAINT ck_amount CHECK (check (amount >= 0))`.
  `Constraint::sql()` is the whole body everywhere else in the workspace.
- [ ] **M27. `ModifyColumn` on SQL Server discards `old_column`** (`sqlserver/mod.rs:77-90`) →
  default changes silently lost, and a Sequence column emits
  `ALTER COLUMN id integer identity(1,1)`, a syntax error.
- [ ] **M28. Generated migrations have no transaction wrapper** — none of the three generators
  emit `BEGIN`/`COMMIT`, so a mid-file failure leaves a half-migrated database.
- [ ] **M29. `databaseType`-scoped views and constraints leak into the wrong dialect's migration**
  (`diff_engine.rs:241,251` use `all_views()`, and constraints are never filtered).
- [ ] **M30. Diagrams emit a dangling entity for schema-qualified relations.** [v] `entity UNIT`
  is declared, `KBI }o--o| TEST_UNIT` is referenced. Both `.mmd` and `.puml`.
- [ ] **M31. Diagram identifier map silently merges same-named tables**
  (`common/safe_identifier.rs:28-38`) — `map.insert(name, candidate)` on a duplicate name
  overwrites, so `sales.users` and `auth.users` both resolve to `USERS_2`.
- [ ] **M32. Diagram cardinality is derived from the delete action, not nullability**
  (`mermaid_generator.rs:94`, `plantuml_generator.rs:120`) — wrong in both directions.
- [ ] **M33. Reverse engineer: cross-schema FKs, expression/partial/covering indexes,
  materialized views.** `relations.rs:28-47` filters only the FK-owning table by schema and
  stores `to_table` bare → dangling relation → downstream `panic!("Unable to locate a
  table…")`. `keys.rs:72-86` ignores `indpred`/`indexprs`/`indnkeyatts`, so a partial unique
  index round-trips as a zero-column key and an `INCLUDE` index gains phantom key columns;
  `relkind = 'r'` also excludes partitioned tables that `tables.rs:8` includes.
  `views.rs:7-16` decodes `view_definition` as non-nullable (NULL for a non-owner role →
  hard failure) and `information_schema.views` excludes materialized views.
  `keys.rs:114-119` promotes a lone nullable `UNIQUE` to `PRIMARY KEY`, inventing a
  `NOT NULL` the source database doesn't have.
- [ ] **M34. `ON DELETE` mapping is inconsistent across the three crates.** Create path:
  `DoNothing → "no action"`. Migration path: `DoNothing → "ON DELETE RESTRICT"`. Reverse
  engineer: `"SET DEFAULT" => SetNull` (`relations.rs:66-73`) — so a real `ON DELETE SET
  DEFAULT` round-trips as `SET NULL`, nulling the column instead of restoring the default.
- [ ] **M35. `schema-reverse-engineer` supports PostgreSQL only** while the rest of the workspace
  supports three dialects.
- [ ] **M36. `schema-diff` and `schema-migration-generator` ship no binary.** Only four crates
  declare `[[bin]]`, so the diff→migration workflow is reachable only from tests — which is
  precisely why C7, H19 and H20 went unnoticed.

---

## Low / hygiene

- [ ] **L1.** `schema-sql-generator/src/main.rs` is panic-driven: `File::create(output_path).expect("")`
  (line 116, empty message), `database_type.parse().unwrap()` (118),
  `.expect("failed to parse the schema")` (136); a bad `--postgresql-version` silently
  becomes `0` (110-113). `schema-diagram-generator/src/main.rs:56-64` is the same, also
  panicking on `--schema-file ""` and silently overwriting a neighbouring file (no `--output`).
- [ ] **L2.** The checked-in reference SQL (`schema-parser/tests/resources/schema-parser-test-schema.sql`)
  is stale and contains the broken C3 and C5 output — no test compares against it.
- [ ] **L3.** `Relation::disable_usage_checking()` has **no** call site anywhere:
  `disableUsageChecking="true"` is parsed, validated, stored and ignored (`convert.rs:306-317`).
- [ ] **L4.** `common/relation_generator.rs:80-91` never emits an `on update` clause, and
  `DoNothing` maps to `"no action"` — identical to `Enforce`, so the two declared relation
  types are indistinguishable in output.
- [ ] **L5.** `postgres_trigger_generator.rs:52-54` gates on `table.primary_key().is_some()`, so
  an explicit `<delete>` trigger on a PK-less table is silently dropped even though the
  emitted body never uses the PK.
- [ ] **L6.** `roxml_parser.rs:161-164` — `parse_check_node` uses `node.text()` (first text child
  only) while everything else uses `collect_text`, so `<check>a<!-- note -->b</check>` yields
  only `a`.
- [ ] **L7.** `KeyBuilder::new(KeyType::Unique)` (`builder/key.rs:53-63`) produces a `Key` whose
  `is_unique()` is `false` — a trap for any future code keying off `is_unique()`.
- [ ] **L8.** `MigrationStatus` (`migration.rs:32-56`) is exported and used by nothing — every
  status is a bare `"success"`/`"failed"`/`"pending"` string literal, exactly the typo class
  the enum would prevent. `SchemaInstallerError::Generation` is never constructed.
- [ ] **L9.** `installer.rs:49-70` writes generated SQL to a predictable path in the shared
  `temp_dir()` with `File::create` (no `O_EXCL`) and discards the cleanup failure — a local
  attacker can pre-create/symlink it and have arbitrary DDL executed with migration
  privileges. `tempfile` is already a dev-dependency. Related: `PrintWriter`'s `BufWriter` is
  flushed only by `Drop`, which ignores errors, so a failed flush yields **truncated** SQL
  that is then checksummed and executed as if complete.
- [ ] **L10.** `migrator.rs:243` uses `split('/').next_back()` — on Windows paths `info` prints
  the whole path and wrecks the `{:<30}` column. Use `Path::file_name()`.
- [ ] **L11.** `connection.rs:167,194,222` `ORDER BY installed_at`, but SQLite's
  `CURRENT_TIMESTAMP` has 1-second resolution → same-second migrations return in arbitrary
  order. `ORDER BY id` is deterministic and free.
- [ ] **L12.** `sql_split.rs:313` requires the line to be exactly `GO`; T-SQL's `GO 5` and
  `GO -- comment` are sent verbatim. `sql_split.rs:137-149` allows a leading digit in a
  dollar-quote tag, which PostgreSQL does not, so `$1$…$1$` swallows statements.
- [ ] **L13.** `repair` prints "Deleted failed migrations" unconditionally even when zero rows
  matched (`migrator.rs:356-363`). `migration.rs:89` silently skips subdirectories (Flyway
  recurses by default).
- [ ] **L14.** `xml_writer.rs:114` escapes only `&<>"'`, so a multi-line default expression keeps
  a literal newline inside an attribute value; XML attribute-value normalization turns it
  into a space on re-parse, altering the SQL. Needs `&#10;`.
- [x] **L15a.** [v] `diff_engine.rs:14-35` never inspects triggers, functions, procedures,
  `other_sql`, `enum_types` or `initial_data` — so adding a value to an enum produces an
  empty change set and `ALTER TYPE … ADD VALUE` can never be generated. Fixed: `schema-diff`
  now diffs all six categories (new `SchemaChange` variants + `diff_engine.rs` functions), and
  `schema-migration-generator`'s three backends emit real SQL for them (Postgres native
  `ALTER TYPE ... ADD VALUE`/`CREATE TYPE`; SQL Server regenerates the CHECK constraint on
  every column referencing a modified enum; SQLite gets a manual-rebuild comment, matching its
  existing `AddConstraint`/`DropConstraint` behavior). Triggers reuse a new
  `TriggerGenerator::output_triggers_for_table` so migrations regenerate a table's full,
  already-idempotent trigger set rather than re-deriving it. Functions/procedures/other_sql/
  initial_data follow the existing `AddView`/`DropView` add-or-drop-by-content pattern.
- [ ] **L15b.** `xml_writer.rs:36-46,73-84` (schema-reverse-engineer) never emits `<triggers>`,
  `<functions>`, `<procedures>`, `<initialData>`, `<aggregations>`, `noExport` or
  `lockEscalation`. Deliberately left open: `schema-reverse-engineer`'s DB reader doesn't
  introspect any of these from a live database yet, so this is currently dead code, not real
  data loss - fixing it for real needs Postgres catalog introspection work first.
- [ ] **L16.** No CI: no `.github/` at all, `.githooks/` is empty. 326 tests and `cargo clippy`
  run only when someone remembers, and the 28 `#[ignore]`d PostgreSQL/SQL Server integration
  tests never run — which is how C6 shipped.
- [ ] **L17.** `.DS_Store` and `.idea/*` are tracked in git despite `.gitignore` listing `.idea/`.
- [ ] **L18.** Docs drift: AGENTS.md and README.md:276 show `--database-type postgres`; the
  accepted value is `postgresql` [v — clap rejects `postgres`]. README:283 says the output is
  `schema-postgres.sql`; it is `schema-postgresql.sql`. README:49 claims `info` shows
  checksums; it does not. AGENTS.md claims `uuid-ossp` is emitted (H8).
- [ ] **L19.** Clippy: `large_enum_variant` on `AnyPool` (864 bytes), two `type_complexity`
  tuples, `should_implement_trait` on `MigrationStatus::from_str`, `too_many_arguments` on
  `SqlGenerator::new`, `module_inception` in schema-parser.
- [ ] **L20.** Flyway gaps in the installer CLI: no `baseline` (so this can never be adopted on
  an existing database without hand-inserting rows), no `--out-of-order`, `--target`,
  `--dry-run`, `clean`, `undo`, repeatable migrations, or configurable history-table name.
  `migrator.rs:16,24` also leave a 9-minute unrecoverable window between the 60 s lock
  timeout and the 600 s stale-pending threshold, with no `--force`/`--lock-timeout` override.

---

## Status of the previous audit's claimed fixes

- **#2 (SQLite FKs)** — regressed into H1: the fix is unreachable from the pipeline, so
  SQLite output now has no foreign keys at all.
- **#6 (static dispatch)** — fixed for functions/procedures only; the same shape remains in
  `TableGenerator` (where it causes H1), `IndexGenerator`, and `OtherSqlGenerator` (M10).
- **#15 (identifier length)** — byte-vs-char limit mismatch remains (M8).
- **#16 (SQL escaping)** — only string *literals* were addressed; identifiers are still
  interpolated raw everywhere (C5).
- **#22/#23 (typo'd attributes)** — closed for string/enum attributes, still open for
  numerics, booleans, and the two root-element mode attributes (H4, H5).
- **#32 (validate gaps)** — duplicate table/column names and key/index column existence
  remain unchecked, and `validate()` also misses `Enum`-without-`enumType`, invalid or
  unsupported array element types, and dialect capability — all live panics (H23, M11).
- **The remaining unchecked box** ("no `.sql`-execution-based tests") is directly responsible
  for C1, C2, C3, C5, H1, H3, H6 and H16 — every one of them is currently asserted as
  *expected* output by a passing string-equality test or fixture.

---

## Remediation plan

Ordered so each step is independently shippable. **Step 0 first** — it is what makes the
rest verifiable and stops this class of defect recurring.

**Tracking convention:** every bullet below is a checkbox keyed to the finding IDs it covers.
Check a box only when *all* of its IDs are marked `[x]` above; when a plan bullet's fix note
above records a caveat (partial fix, skipped at user's request, blocked by environment), the
box stays unchecked here too and the caveat is repeated inline — the plan should never claim
more progress than the findings section it's derived from.

### Step 0 — Test what the tools actually produce
1. [ ] `schema-sql-generator/tests/sqlite_execution_tests.rs`: generate DDL from
   `schema-parser-test-schema.xml`, **execute** it via `rusqlite` in-memory, insert a row
   into a Sequence-PK table, and assert FKs exist via `PRAGMA foreign_key_list`. This one
   test catches C1, C2, C3, C4, C5 and H1. Not created — no such file exists yet.
2. [ ] `schema-migration-generator/tests/pipeline_tests.rs`: run
   `SchemaDiffEngine::diff(old, new)` → `create_generator(…).generate(…)` for a new table
   and execute the result. Every existing test hand-builds a `ChangeSet`, which is why C7,
   H19 and H20 went unnoticed. Not created — no such file exists yet.
3. [ ] Un-`#[ignore]` the PostgreSQL and SQL Server integration tests behind a
   docker-compose/testcontainers fixture. C6 would have failed on the first run. Still
   `#[ignore]`d (`sqlserver_integration_tests.rs`) — blocked in this sandbox by SQL Server's
   Linux container crashing under QEMU emulation on ARM64 (see C6); worth revisiting on a
   native x86_64 host/CI runner.
4. [x] Fix the tests that assert broken output as expected: `sqlite_table_generator.rs:92`
   (`auto_increment`), `postgres_column_type_generator.rs:194` (`char(0)`), and regenerate
   the stale reference `.sql`. Done as part of C2/H1 and H3: the SQLite test now asserts
   `"id integer primary key autoincrement"`, and neither `postgres_column_type_generator.rs`
   nor `sqlite_column_type_generator.rs` asserts a zero length anymore.

### Step 1 — Make SQLite output runnable (C1, C2, H1)
- [x] `sqlite_column_type_generator.rs:23-29`: emit `integer primary key autoincrement` for
  `Sequence`/`LongSequence` and suppress the separate `constraint pk_x primary key (…)`
  clause for that table (SQLite requires the PK inline for rowid aliasing). — Fixed with C2.
- [x] Fix the dead override: give `SqliteTableGenerator` its own self-dispatching
  `output_tables`/`output_table`, mirroring `SqlServerTableGenerator::output_tables`
  (`sqlserver_table_generator.rs:35-50`). Do the same for `PostgresTableGenerator` and
  restore its `output_table` to the full five-call sequence. — Fixed with H1.
- [ ] Sweep every trait for the same "delegate to `Default*` and lose self-dispatch" shape —
  M10 lists the three that remain. This is the pattern's **third** appearance (BUGS #6, H1);
  consider making the default implementations take `&dyn Trait` so it can't recur. — Two
  more instances were found and fixed opportunistically while fixing H1 (see H1's note), but
  M10's three remaining instances (`DefaultIndexGenerator`, `OtherSqlGenerator`,
  `PostgresFunctionGenerator`) are still open.
- Note: C1 itself remains open — see C1's own entry above for the one residual failure
  (a doubled `default default` on the fixture's `char` column) not covered by this step.

### Step 2 — Cross-dialect DDL correctness (C3, C4, C5, H2, H3, H6, H7, H8, H15–H18)
- [x] C3: wrap `column.check_constraint()` in `check(…)` unless it already starts with `check`.
- [x] C4: emit schema DDL in the header — `create schema if not exists` (PostgreSQL), the
  `if not exists (select … from sys.schemas) exec('create schema …')` form (SQL Server);
  for SQLite either flatten `schema.table` → `schema_table` or reject multi-schema models.
- [ ] C5: add a per-dialect `quote_identifier` (`"x"` / `[x]` / `"x"`) and route every
  table/column/constraint/index name through it. A mechanical sweep of
  `column_generator.rs`, `key_generator.rs`, `index_generator.rs`, `relation_generator.rs`.
  — Skipped at user's request (see C5); direction changed to "error on a bad identifier"
  rather than auto-quote, still unresolved.
- [x] H2: parse boolean defaults properly (`1`/`yes`/`on`/`true`, trimmed) and error on
  anything else; let a boolean column have no default.
- [x] H3/H6: reject zero-length `char`/`varchar` in `validate()`, and filter empty strings
  out of the `create table` definition list.
- [x] H7: suffix PostgreSQL trigger names with the operation, matching the SQL Server generator.
- [x] H8: emit `create extension if not exists "pgcrypto"`, then correct AGENTS.md.
- [x] H15: replace bare `sysobjects where name = …` guards with
  `object_id('{schema}.{name}', 'U'|'V'|'TR') is not null`.
- [x] H16: add a `validate()` rule rejecting >1 auto-increment column per table.
- [x] H17: emit all drops first, in reverse-dependency order (or drop FK constraints first).
- [x] H18: filter table constraints by `database_type`, as views and initial data already are.

### Step 3 — Parser strictness (H4, H5, L6)
- [x] Make `attr_i32`/`attr_f64`/`attr_bool` return `Err` on an unparseable value instead of
  `None` (H4), and replace the two `.unwrap()`s in `convert.rs:35,41` (H5). These were the
  last silent holes left by the BUGS #22/#23 sweep, and they fed directly into H3.
- [ ] L6: `roxml_parser.rs`'s `parse_check_node` still uses `node.text()` instead of
  `collect_text`, so a `<check>` with a comment or nested element loses everything after it.
  Not addressed.

### Step 4 — Unbreak SQL Server in the installer (C6, M21, M22)
- [x] `version NVARCHAR(100)` (and bounded types for the other columns), read `installed_at`
  as `NaiveDateTime`, and switch the default/comparison to `SYSUTCDATETIME()` — done as C6.
  Caveat: could not be confirmed end-to-end in this sandbox (SQL Server's Linux container
  crashes under QEMU on this ARM64 host); `sqlserver_integration_tests.rs` is still
  `#[ignore]`d pending a native x86_64 run (tracked jointly with Step 0 item 3).
- [ ] M21: honour `Encrypt=` from the ADO connection string instead of forcing TLS on.
- [ ] M22: replace the error-string sniffing in `is_installed`/`get_installed_version` with
  an `ensure_migration_table` call, so SQL Server's `Invalid object name` is handled too.

### Step 5 — Installer semantics (H9–H14, M13–M22)
- [x] Write the tracking row inside the migration's transaction for PostgreSQL/SQLite (H9).
- [x] Make `validate` fail on `failed`/`pending` rows and on resolved-but-unapplied
  migrations, matching what `migrate` enforces (H10).
- [x] Skip non-`V` files with a warning instead of aborting (H11).
- [x] Call `ensure_migration_table` in `repair` (H12).
- [x] Hard-error on duplicate versions at load time (H13).
- [x] Use `execute_transactional` in the `install` path and scope `check_if_installed` to
  `RESERVED_INSTALL_VERSION` (H14).
- [ ] Store the script filename, add a `description` column, use it in `info` (M13, M14).
- [ ] Give `sql_split` nested-comment depth, `E''` backslash handling and end-state validation
  (M15–M17); reject version strings the comparator can't parse (M19); stop swallowing errors
  as "pending" (M20).

### Step 6 — Decide the fate of the diff pipeline (C7, H19, H20, M23–M29, M36)
Close to a rewrite of `schema-diff`'s change model, and the right moment to decide whether
the crate is worth keeping in its current form. **C7 itself is skipped at user's request**
("let's skip this for now") — none of the following has been started. Minimum viable:
- [ ] Give `AddTable` a full `Table` payload and route it through the existing
  `schema-sql-generator` table generator rather than a second, divergent type mapping —
  that single change fixes C7, M23 and M24 together and removes the duplicate mapping
  causing permanent diff churn.
- [ ] Share the constraint/index/FK naming helpers with `schema-sql-generator` (H20).
- [ ] Compare `relation_type`, key uniqueness, constraint bodies and view bodies (H19).
- [ ] Emit rename changes, or delete the dead `RenameTable`/`RenameColumn` variants and
  document that renames are unsupported — silently emitting `DROP TABLE` is the worst of
  the three.
- [ ] Guard destructive statements behind a flag; wrap output in a transaction (M28).
- [ ] Ship a binary, or fold the functionality into `schema-installer` (M36).

### Step 7 — Model, diagrams, reverse engineer, hygiene
- [x] H21: drop the `to_lowercase()` in `Table::column`/`has_column`; settle on one
  case-folding convention across `Schema::table_index`, `Table::column` and
  `Key::contains_column`.
- [ ] H22/M11/M12: finish `Schema::validate()` (duplicate names, key/index columns, relation
  target columns, enum/array type resolution — H23's panic sites); make schema lookup
  case-insensitive.
- [ ] M30/M31/M32: resolve diagram relation endpoints through `find_table_by_qualified_name`
  and emit the same entity id used for the declaration; key the identifier map by qualified
  name; derive cardinality from the FK column's `required()`.
- [x] H24: keep the schema name, write keys/indexes for PK-less tables, sort key output, and
  skip unsupported column types with a warning.
- [ ] M33/M34: cross-schema FKs, expression/partial/covering indexes, materialized views, and
  align the `ON DELETE` mapping across all three crates.
- [ ] L1: convert both panicking `main.rs` files to `Result`-returning, like the other binaries.
- [ ] L16: add a GitHub Actions workflow — `cargo fmt --check`,
  `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, plus a services job
  for the integration tests.
- [ ] L17/L18/L19: untrack `.DS_Store`/`.idea`, correct the docs, clear the clippy warnings.
  Partial: L18's AGENTS.md `uuid-ossp` claim was already corrected as part of H8.

---

## Verification

```bash
# Baseline — generate all three dialects
for db in postgresql sqlite sqlserver; do
  cargo run -p schema-sql-generator -- --database-type $db \
    --schema-file schema-parser/tests/resources/schema-parser-test-schema.xml \
    --foreign-key-mode relations --boolean-mode native
done

# C1/C2/C3/C4/C5/H1 — must exit 0 and contain foreign keys after the fix
sqlite3 /tmp/t.db < schema-parser/tests/resources/schema-parser-test-schema-sqlite.sql
grep -c 'foreign key' schema-parser/tests/resources/schema-parser-test-schema-sqlite.sql

# C5/H2/H3/H4/H6 — the edge-case schema used in this review:
#   a table named `order`, a column named `select`, boolean default="1",
#   char with no length, length="1O0"

# C3/C4/H7/H8/H15/H16/H17 — run the scripts against real servers
docker run --rm -d -e POSTGRES_PASSWORD=pw -p 5432:5432 postgres:16
psql "postgres://postgres:pw@localhost/postgres" -v ON_ERROR_STOP=1 \
  -f schema-parser/tests/resources/schema-parser-test-schema-postgresql.sql
#   then: insert into a Uuid-defaulted column (H8); check pg_trigger for two distinct
#   triggers on parenttable (H7); re-run the whole script twice (H17).
docker run --rm -d -e ACCEPT_EULA=Y -e SA_PASSWORD=Pw12345! -p 1433:1433 \
  mcr.microsoft.com/mssql/server:2022-latest        # C6, H15, H16

# C7/H19/H20 — diff pipeline
#   Two DatabaseModels differing by one new table; run diff -> generator, execute the
#   output. Today this produces `CREATE TABLE orders ();`.

# M30/M31 — no undeclared or duplicated entity ids
cargo run -p schema-diagram-generator -- --format plantuml \
  --schema-file schema-parser/tests/resources/schema-parser-test-schema.xml

# H10/H11/H12/M13/M14 — installer lifecycle against a real SQLite db
#   migrate a set with a failing V3, then `validate` (must exit 1), `repair`, `migrate`;
#   add an out-of-order V1.5 and re-run `validate` (must exit 1);
#   drop an `R__x.sql` into the directory and confirm `info` still works;
#   run `repair` against an empty database.

cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```
