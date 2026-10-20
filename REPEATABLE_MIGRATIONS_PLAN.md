# Plan: repeatable (`R__`) migrations in schema-installer

## Semantics (Flyway-compatible)

- A file named `R__{description}.sql` has no version.
- It runs when it has never been applied, or when its checksum differs from the last applied one.
- All pending repeatables run **after** all pending versioned migrations in the same `migrate` call, in alphabetical order by name. There is no ordering config; users control order by naming (`R__01_views.sql`, `R__02_grants.sql`).
- A changed repeatable is not a checksum error. It is simply pending again (`Outdated` in `info`).
- Out-of-order checks never apply to repeatables.
- `--target <version>`: repeatables are **skipped**. Unverified against Flyway; chosen conservatively so a rollout deliberately stopped at version N does not run repeatables written against a newer schema.

## Decisions

- **Storage:** separate table `schema_repeatable_migration(id, name UNIQUE, script_path, checksum, execution_time_ms, installed_at, status, tool_version)`. One row per repeatable, updated in place on a re-run. `schema_migration` is not touched.
  - Why: nearly every function in `migrator.rs` assumes `schema_migration` rows are numeric versions (`compare_versions`, `highest_applied`, out-of-order check, checksum-mismatch loop, missing-source check). Keeping repeatables out removes the need to filter them everywhere, and old tool versions never see the rows.
  - Reverses the earlier consolidation to a single tracking table (`tracking.rs`); justified because repeatables have no order and re-run on change.
  - Trade-off: only the latest run is kept (Flyway keeps every run). Append-only history is out of scope for v1.
- **`repair`:** matches Flyway. Deletes failed and stale-pending rows and rewrites changed checksums for repeatables too; it does not force a re-run.
- **Errors, not silent skips:** `R.sql`, `R__.sql`, `R_foo.sql` and duplicate names are hard errors. `U__` files keep the current warn-and-skip.
- **Baseline / `install`:** no change. Repeatables run on the first `migrate` after a baseline.

## Steps (TDD, one commit-sized chunk each)

1. **Model and parsing** (`migration.rs`)
   - Add `MigrationKind { Versioned, Repeatable }` and `kind` on `Migration`.
   - Add a `Migration::versioned(..)` constructor to limit the diff at existing struct literals (tests, fixtures).
   - Split `parse_migration_filename` into versioned and repeatable branches.
   - Unit tests: valid names, malformed names, duplicate names, `U__` skip, `.conf` sidecar (`executeInTransaction`) on a repeatable.
2. **Tracking DDL** (`tracking.rs`, `connection.rs`)
   - Add `repeatable_migration_ddl` for Postgres, SQL Server (bounded column types) and SQLite.
   - `ensure_migration_table` creates it. DDL tests mirror the existing ones.
3. **Pool methods** (`connection.rs`)
   - Parametrize insert-pending, update-status, delete-failed, delete-stale-pending and get-applied by a `HistoryTable` enum instead of duplicating them.
   - Add the conditional re-pend `UPDATE ... SET status='pending', checksum=? WHERE id=? AND checksum=<old> AND status='success'` and check rows-affected so only one instance wins a re-run. Mind `$1` vs `?` placeholders per dialect.
4. **`migrate`** (`migrator.rs`)
   - `resolve_pending_migrations` returns versioned migrations unchanged, then pending repeatables (new, or checksum differs).
   - Add repeatable lock handling alongside `wait_for_slot`.
   - Reuse `execute_migration` as-is (transactional and non-transactional paths).
   - SQLite integration tests:
     - new repeatable runs after the versioned ones
     - unchanged one is a no-op
     - changed one re-runs
     - failed one blocks the next run; `repair` clears it
     - `--target` skips repeatables
     - two concurrent runs execute a changed repeatable only once
5. **Other commands**
   - `info`: merge both tables; statuses `Pending`, `success`, `Outdated`.
   - `validate`: missing source for a repeatable is an issue; a changed one is not.
   - `pending-check`: also true for new or changed repeatables.
   - `dry_run`: label repeatables in the output.
   - `repair`: handle the second table.
6. **Dialect coverage**
   - Add Postgres and SQL Server cases to the existing gated integration suites.
   - Test that repeatables run on the first `migrate` after a baseline.
7. **Docs**
   - Update `AGENTS.md` and `README.md` (file format, ordering, re-run rules).
   - Update the single-table note about `tracking.rs`.
   - Add a closing entry to `BUGS_AND_GAPS_2026-09-08.md` for the repeatable-migrations gap (~line 1335).

## Status (2026-10-20)

Steps 1–7 are implemented (uncommitted). Step 6: one dialect-neutral scenario (`tests/common/mod.rs`) runs against Postgres (`RUN_DOCKER_TESTS=1`) and SQL Server (`--ignored`); both pass. Deviation from the plan: repeatables use a separate `RepeatableMigration` type and `MigrationSource::repeatable_migrations()` rather than a `kind` field on `Migration`; `EmbeddedMigrationSource` carries them in a new `repeatables` field (a breaking change to its struct literals, which now use `..Default::default()`).

## Open item

- Verify Flyway's `target` behavior for repeatables (docs / context7) before implementing step 4's `--target` handling.
