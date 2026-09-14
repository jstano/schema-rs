use schema_model::model::column::Column;
use schema_model::model::constraint::Constraint;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::view::View;

#[derive(Debug, Clone)]
pub enum SchemaChange {
    AddTable {
        table_name: String,
    },
    DropTable {
        table_name: String,
    },
    RenameTable {
        old_name: String,
        new_name: String,
    },
    AddColumn {
        table_name: String,
        column: Column,
    },
    DropColumn {
        table_name: String,
        column_name: String,
        rename_candidates: Vec<String>,
    },
    RenameColumn {
        table_name: String,
        old_name: String,
        new_name: String,
    },
    ModifyColumn {
        table_name: String,
        old_column: Column,
        new_column: Column,
    },
    AddKey {
        table_name: String,
        key: Key,
        /// 1-based position of `key` among the sibling keys the create path numbers it
        /// with - unique keys are numbered among the table's other unique keys, indexes
        /// among the table's other indexes - so a migration can reproduce the exact
        /// `ak_<table><n>` / `ix_<table><n>` name the create path would use.
        ordinal: usize,
    },
    DropKey {
        table_name: String,
        key: Key,
        /// See `AddKey::ordinal`.
        ordinal: usize,
    },
    AddConstraint {
        table_name: String,
        constraint: Constraint,
    },
    DropConstraint {
        table_name: String,
        constraint_name: String,
    },
    AddRelation {
        relation: Relation,
        /// 1-based position of `relation` among its table's relations, matching the
        /// create path's `fk_<table><n>` numbering.
        ordinal: usize,
    },
    DropRelation {
        relation: Relation,
        /// See `AddRelation::ordinal`.
        ordinal: usize,
    },
    AddView {
        view: View,
    },
    DropView {
        view_name: String,
    },
}
