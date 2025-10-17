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

## Workspace crates

- schema-model: core data structures (tables, columns, relations, views, functions, procedures, triggers, other SQL, versions, etc.).
- schema-parser: parsing/processing helpers (WIP).
- schema-sql-generator: backend(s) to create SQL from the model (WIP).
- schema-installer: utilities for applying schemas/migrations (WIP).

## License

Apache-2.0 or MIT, at your option.
