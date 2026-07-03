# schema-diff

Computes structural differences between two database schemas. Produces a `ChangeSet` describing what tables, columns, keys, constraints, relations, and views were added, dropped, renamed, or modified.

## Usage

```rust
use schema_diff::SchemaDiffEngine;

let change_set = SchemaDiffEngine::diff(&old_schema, &new_schema);

for change in change_set.changes() {
    println!("{:?}", change);
}
```

## Change Types

The `SchemaChange` enum represents a single structural change:

- **Tables**: `AddTable`, `DropTable`, `RenameTable`
- **Columns**: `AddColumn`, `DropColumn`, `RenameColumn`, `ModifyColumn` (type/length/required/default changes)
- **Keys**: `AddKey`, `DropKey` (primary, unique, index)
- **Constraints**: `AddConstraint`, `DropConstraint` (check constraints)
- **Relations**: `AddRelation`, `DropRelation` (foreign keys)
- **Views**: `AddView`, `DropView`

### Rename Detection

The `DropColumn` variant includes a `rename_candidates: Vec<String>` heuristic: columns of the same type that disappeared in the old schema and appeared in the new schema are flagged as potential renames, allowing downstream tools to suggest `RENAME COLUMN` rather than dropping and re-adding.

## Ordering

Changes are emitted in a fixed order to respect dependencies:

**Drop Phase** (prevent constraint violations):
1. Views
2. Relations (foreign keys)
3. Keys (primary, unique, indexes)
4. Constraints (check constraints)
5. Columns
6. Tables

**Add Phase** (respect creation dependencies):
1. Tables
2. Columns
3. Modify columns
4. Keys
5. Constraints
6. Relations
7. Views

## Library API

```rust
pub struct SchemaDiffEngine;

impl SchemaDiffEngine {
    pub fn diff(old: &Schema, new: &Schema) -> ChangeSet { ... }
}

pub struct ChangeSet { ... }

impl ChangeSet {
    pub fn new() -> Self { ... }
    pub fn add_change(&mut self, change: SchemaChange) { ... }
    pub fn changes(&self) -> &[SchemaChange] { ... }
    pub fn is_empty(&self) -> bool { ... }
    pub fn len(&self) -> usize { ... }
}
```

## Part of schema-rs

See the [workspace README](../README.md) for an overview of the full schema-rs toolchain. Typically used with `schema-migration-generator` to convert diffs into migration SQL.
