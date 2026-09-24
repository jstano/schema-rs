# Codebase Bug & Gap Analysis — schema-rs

Whole-workspace audit for bugs and gaps, generated 2026-08-21. Findings are
grouped by severity and checkable so they can be worked through over time.
Each item lists the file(s)/line(s) involved and a concrete failure scenario.

---

## Critical — crashes or always-produces-broken-output on realistic input

- [x] **1. SQLite generation always panics.**
  `schema-sql-generator/src/sqlite/sqlite_procedure_generator.rs:15-21` unconditionally
  `panic!`s in `output_procedures()`. `DefaultSqlGenerator::output_sql()`
  (`common/sql_generator.rs:111-132`) calls it under the default `OutputMode::All`.
  Running the CLI against `--database-type sqlite` with default flags always panics,
  even on an empty schema. No test exercises the full `SqliteGenerator` pipeline.

- [x] **2. SQLite FK relation SQL is invalid syntax.**
  `sqlite/sqlite_relation_generator.rs` delegates to `common/relation_generator.rs:59-70`,
  which emits `alter table ... add constraint ... foreign key ...` — SQLite's
  `ALTER TABLE` has no such form (FKs must be inline in `CREATE TABLE`). Any SQLite
  schema with a relation under the default `ForeignKeyMode::Relations` generates SQL
  that fails to execute. The crate's own passing unit test asserts this broken output.

- [x] **3. Postgres array columns always panic — `element_type` is dead code.**
  `postgresql/postgres_column_type_generator.rs:70-82` (`array_sql`) matches on
  `column.column_type()`, which is always `ColumnType::Array` itself at that call
  site (that's how dispatch got there) — the real element type lives in
  `Column.element_type: Option<String>` and is never read. Every `Array` column on
  Postgres (the one dialect with native array support) panics. Untested.

- [x] **4. Malformed XML crashes the parser instead of returning `Err`.**
  `parse_database_xml` promises `Result<DatabaseModel, String>`, but:
  - A dangling relation target (typo'd table name, or a schema not declared) panics
    via `DatabaseModel::find_schema_mut` (`.expect("Schema not found")`,
    `schema-model/src/model/database_model.rs:62-72`) and
    `Schema::get_table_mut`/`table_index` (`schema.rs:69-78`, `panic!(...)`).
    (`schema-parser/src/parser/convert.rs:259-288`)
  - An invalid column `type=` attribute panics via
    `schema-parser/src/parser/table_parser.rs:60-61`
    (`.unwrap_or_else(|e| panic!(...))`).
  - `Schema::validate()`'s SETNULL check panics instead of returning its declared
    `Vec<String>` error if `relation.from_column_name()` doesn't exist on the table
    (`schema.rs:121-142` → `table.column()` panics).

- [x] **5. `required="TRUE"` (capitalized) is silently parsed as *not required*.**
  `schema-parser/src/parser/roxml_parser.rs:371-377` only matches lowercase
  `"true"/"1"/"yes"/"on"`; any other casing silently defaults to `false`. This is a
  silent data-integrity bug — a schema author's explicit `required="TRUE"`
  declaration is dropped with no error.

---

## High

- [x] **6. SQL Server function drop-guard override is unreachable.**
  `sqlserver/sqlserver_function_generator.rs:19-35` overrides `output_function` to
  add an `if exists (...) drop function` guard, but the real pipeline entry point
  (`DefaultFunctionGenerator::output_functions`, `common/function_generator.rs:33-49`)
  calls `output_function` via static dispatch on the *concrete* default type, not
  through the trait object — the override never runs. (The crate has a test that
  literally asserts this buggy behavior by name.) Re-running generated SQL against
  an existing function fails with "already an object named ...".

- [x] **7. SQL Server trigger generator panics on unqualified relation targets.**
  `sqlserver/sqlserver_trigger_generator.rs:104-106,135-138,152-155,203-206` resolves
  the FK target table via `to_table_name().split('.').next()/.next_back()` instead of
  `DatabaseModel::find_table_by_qualified_name` (used correctly elsewhere). For an
  unqualified `to_table_name` (the common case), the whole string is passed as the
  *schema* argument, panicking with `"Schema not found"` unless a schema of that name
  happens to exist. Only the schema-qualified case is tested.

- [x] **8. Composite primary keys break delete/update triggers (Postgres + SQL Server).**
  `postgres_trigger_generator.rs:44-55` / `sqlserver_trigger_generator.rs:44-55`
  (`get_primary_key_column`) keep only the *last* column of a composite PK via
  `.and_then(|mut cols| cols.pop())`. Under `ForeignKeyMode::Triggers`, generated
  triggers filter using only that one column — logically wrong for any multi-column
  PK table.

- [x] **9. Migrations may be applied out of order.**
  `Migrator::migrate` (`schema-installer/src/migrator.rs:128-189`) never sorts
  `source.migrations()` before applying, unlike `info()` which explicitly re-sorts
  with `compare_versions`. `EmbeddedMigrationSource::migrations()`
  (`migration.rs:130-133`) returns whatever order the caller supplied. A
  `vec![v3, v1, v2]` embedded source silently runs V3 before V1/V2.

- [x] **10. `repair()` doesn't clear stale "pending" rows from a crashed process.**
  `migrator.rs:336-370` only deletes rows with `status = 'failed'`. A process killed
  mid-migration leaves a `"pending"` row forever, permanently wedging every future
  `migrate()` call behind the lock-wait timeout with no documented recovery.

- [x] **11. XML writer doesn't escape `]]>` inside CDATA.**
  `schema-reverse-engineer/src/xml_writer.rs:67,222` emit raw SQL text inside
  `<![CDATA[...]]>`. The literal sequence `]]>` is illegal inside XML character
  data anywhere (not just outside CDATA) — any view/constraint SQL containing it
  (e.g. array-slice syntax) produces non-well-formed XML with no error raised.

- [x] **12. Unrecognized `databaseType` values are silently dropped or misapplied.**
  `str_to_database_type` (`convert.rs:233-240`) returns `None` for any typo'd value;
  every call site treats `None` as "discard silently" for functions/procedures/
  otherSql/triggers/constraints. For **Views** it's inverted and worse:
  `Schema::views(db_type)` treats `None` as "applies to every database type" — a
  Postgres-only view with a typo'd `databaseType` silently gets emitted for every
  database.

- [x] **13. Case-insensitivity is inconsistent across the model.**
  AGENTS.md documents case-insensitive table/column lookup, but:
  - `Schema::add_enum_type`/`get_enum_type` (`schema.rs:103-107,165-168`) are
    case-sensitive and `get_enum_type` **panics** on a case-mismatched lookup.
  - `Key::contains_column` (`schema-model/src/model/key.rs:86-88`) is case-sensitive
    while `Table::column`/`has_column` are not.
  Neither is covered by any test.

---

## Medium

- [x] **14. Constraint-name truncation panics on multi-byte UTF-8 table names.**
  `common/key_generator.rs:54` and `common/relation_generator.rs:41` slice strings
  by raw byte index (`&table_name[..available]`), which panics on non-char-boundary
  truncation of CJK/Cyrillic/emoji names. `common/index_generator.rs:70-74` already
  does this correctly via `.chars().take(n)` — the fix pattern exists but wasn't
  applied consistently.

- [x] **15. Truncated identifiers can still exceed the DB's max length.**
  `relation_generator.rs:40` and `index_generator.rs:69` hard-code a single-digit
  numeric-suffix budget; ≥10 relations/indexes on one table (common for
  self-referencing/audit tables) produces an identifier 1+ chars over the limit —
  most reproducible on SQL Server (`max_key_name_length()` = 32).

- [x] **16. Unescaped string-literal interpolation throughout the SQL generator.**
  No identifier/literal quoting exists anywhere in the crate: extension-check
  username (`postgres_generator.rs:69`), enum value codes, and SQL Server
  `sysobjects` name lookups (`sqlserver_table_generator.rs:65`,
  `sqlserver_view_generator.rs:34`, `sqlserver_trigger_generator.rs:87,190`) are all
  interpolated raw into single-quoted SQL — any value containing a quote breaks the
  generated script (or injects SQL).

- [x] **17. SQL Server `lock_escalation` ALTER isn't schema-qualified.**
  `sqlserver_table_generator.rs:83-93` uses bare `table.name()` while every other
  statement for the same table uses `fully_qualified_table_name` — for a non-`dbo`
  schema this can silently target the wrong object. Untested (all tests use the
  default schema).

- [x] **18. Reserved migration version `"0"` isn't protected against user collision.**
  `parse_migration_filename` (`schema-installer/src/migration.rs:136-176`) accepts
  any non-empty version, including `"0"`, even though `RESERVED_INSTALL_VERSION`
  reserves it for the `install` command's tracking row. A user's `V0__x.sql` collides
  and produces a confusing `ChecksumMismatch`.

- [x] **19. Non-numeric version segments silently mis-sort instead of erroring.**
  `compare_versions` (`migration.rs:178-197`) does
  `.filter_map(|p| p.parse::<u64>().ok())`, silently dropping non-numeric segments.
  A typo'd filename like `V1a__x.sql` parses to an effectively-empty version and
  sorts *before* every real version instead of failing fast.

- [x] **20. SQL Server `GO` batch splitter has no quote/comment awareness.**
  `schema-installer/src/sql_split.rs:169-196` splits purely on lines trimming to
  `"go"`, unlike `split_on_semicolons` which is quote/comment-aware. A multi-line
  string literal or comment containing a standalone `GO` line incorrectly splits
  one batch into two. Also no support for `GO N` repeat-count syntax.

- [x] **21. Diagram generators emit unescaped/unquoted names.**
  `schema-diagram-generator/src/mermaid/mermaid_generator.rs` and
  `plantuml/plantuml_generator.rs` interpolate table/column names directly into
  diagram syntax with no escaping — names with spaces or embedded quotes (plausible
  from reverse-engineered legacy schemas) corrupt the generated diagram.

- [x] **22. Required XML attributes silently default to `""` when absent.**
  `roxml_parser.rs:365-369` (`attr_string_required`) returns an empty string rather
  than erroring when `name`/`type`/`src`/`table`/`column` etc. are missing — nothing
  downstream validates against empty names.

- [x] **23. Silent fallback-to-default on several typo'd enum-like XML values.**
  Unrecognized relation `type` silently becomes `Enforce` (changes referential
  semantics), `lockEscalation` falls back to `Auto`, `frequency` falls back to
  `Monthly`, otherSql `order` falls back to `Top` — all typos silently change
  behavior instead of erroring.

---

## Low

- [x] **24. Installer temp filename can collide under concurrency.**
  `schema-installer/src/installer.rs:41-46` uses only `subsec_nanos()` (discards the
  seconds component, no PID/random suffix) — two concurrent `install()` calls can
  produce the same temp filename and corrupt each other's generated SQL.

- [x] **25. `Migrator::info()` swallows DB errors as "no migrations found".**
  `migrator.rs:200-202` ignores `ensure_migration_table`'s error and
  `unwrap_or_default()`s the applied-migrations query — a real connectivity/
  permissions failure looks identical to an empty, healthy state.

- [x] **26. Checksum normalizes `\r\n` but not lone `\r`.**
  `migration.rs:199-205` — old-Mac-style line endings produce a different checksum
  for logically identical content, causing a spurious `ChecksumMismatch`.

- [x] **27. Malformed enum/relation references panic elsewhere in the SQL generator too**
  (same root cause as #4/#13): `column_type_generator.rs:148`,
  `sqlserver_column_type_generator.rs:115`, `column_constraint_generator.rs:73-75`
  all panic on an unresolvable enum type or relation target rather than surfacing a
  diagnostic `Result`. *(Fixed not by making the generator fallible - `PrintWriter`'s
  write-as-you-go architecture makes that a much larger refactor than warranted here -
  but by catching the underlying cause up front: discovered along the way that
  `Schema::validate()` was never actually called from any production code path.
  Extended it (see #32) to catch these dangling references and wired it into both CLI
  entry points, so a malformed model is rejected with a clear message before it can
  reach these panics.)*

- [x] **28. SQLite views have no idempotency guard** (`create view` with no
  `IF NOT EXISTS` or drop-guard, unlike Postgres's `create or replace` and SQL
  Server's explicit guard) — re-running generated SQL fails if the view already
  exists.

- [x] **29. SQL Server procedures have no drop guard at all** (unlike the — broken —
  attempt for functions in #6).

- [x] **30. Zero-column tables produce invalid DDL** — `common/table_generator.rs:90-109`
  emits `create table t\n(\n)` with nothing between the parens. *(Fixed as part of
  #32's `Schema::validate()` extension - a table with no columns is now a validation
  error, caught before generation.)*

- [x] **31. `PrintWriter` panics on any I/O error** (`common/print_writer.rs:23-49`,
  `.unwrap_or_else(|e| panic!(...))` on every write) — e.g. piping into `head`
  crashes the whole generator instead of exiting cleanly.

- [x] **32. `Schema::validate()` covers only the SETNULL-on-required-column rule.**
  Several invariants implied elsewhere in the model are never checked: relation
  target existence, `enum_type` referencing a declared `EnumType`, `element_type`
  presence/absence consistency with `ColumnType::Array`, duplicate table/column
  names, key/index columns actually existing on the table.
  *(Partially fixed: added `DatabaseModel::validate()` (cross-schema relation-target
  existence) plus `Schema::validate()` checks for undeclared `enum_type` references,
  missing `elementType` on `Array` columns, and zero-column tables (closes #30 too) —
  and wired `.validate()` into both the schema-sql-generator CLI and the
  schema-installer `install` path so these are caught before generation instead of
  panicking deep inside it. Still not checked: duplicate table/column names, and
  key/index columns actually existing on the table — left as a further improvement.)*

- [x] **33. Fragile self-lookup in `Schema::validate()`.** *(Already fixed as a
  side effect of #4: the SETNULL-check rewrite there switched to using the
  already-iterated `table` binding directly instead of re-resolving
  `self.get_table(&relation.from_table_name())`, which is exactly this bug.)*

---

## Test-coverage gaps (cross-cutting)

- [x] No negative/malformed-XML tests anywhere in `schema-parser` — none of #4, #5,
  #12, #22, #23 are covered. *(Fixed alongside #4/#5/#12/#22/#23: each now has a
  dedicated parse-error regression test in `schema-parser/src/parser/parser.rs`.)*
- [x] No test for enum-type or `Key::contains_column` case-(in)sensitivity (#13).
  *(Fixed alongside #13.)*
- [x] No test exercising the full `SqliteGenerator` pipeline end-to-end (#1 would
  have been caught immediately). *(Fixed alongside #1: `output_procedures_*` tests
  now cover the gating logic directly.)*
- [x] No test for `ColumnType::Array` on Postgres (#3). *(Fixed alongside #3.)*
- [x] SQL Server trigger tests only cover schema-qualified relation targets (#7).
  *(Fixed alongside #7.)*
- [ ] No `.sql`-execution-based tests anywhere in `schema-sql-generator` — everything
  is asserted as generated-string equality, which is how #2, #6, and #16 shipped
  unnoticed (the generator's own test for #2 asserts the broken SQL as *expected*
  output). Still true after #1-23 — remains a real gap: consider adding
  `rusqlite`/actual-execution-based assertions for the SQLite generator at least.

---

## Suggested remediation order

1. Fix the three always-crashes/always-broken paths first (#1, #2, #3) — these make
   SQLite and Postgres arrays unusable outright.
2. Convert parser panics to proper `Result` errors (#4, #5, #12, #22, #23) — this is
   one consistent pattern (`attr_string_required`, `unwrap`/`expect`/`panic!` call
   sites in `table_parser.rs`/`convert.rs`) and closes the biggest input-validation
   gap.
3. Fix case-sensitivity inconsistencies (#13) to match the documented model contract.
4. Fix the SQL Server trigger/function bugs (#6, #7, #8) — silent incorrect SQL for
   real schemas.
5. Address the migration-ordering and repair gaps (#9, #10) — data-integrity risk in
   production migration flows.
6. Sweep the remaining Medium/Low items, most of which are localized one-file fixes.

## Verification

- Add/extend unit tests per fix (the crates already use inline `#[cfg(test)]`
  modules — follow existing patterns in each file).
- For SQL-generator fixes, prefer asserting against an actual SQL syntax check where
  feasible (e.g. spin up SQLite in-memory via `rusqlite` to execute generated DDL)
  rather than only string-equality, since string-equality tests are how several of
  these bugs went unnoticed.
- Run `cargo test --workspace` and `cargo clippy --workspace` after each batch of
  fixes.
