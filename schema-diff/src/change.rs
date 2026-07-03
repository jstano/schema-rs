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
    },
    DropKey {
        table_name: String,
        key: Key,
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
    },
    DropRelation {
        relation: Relation,
    },
    AddView {
        view: View,
    },
    DropView {
        view_name: String,
    },
}
