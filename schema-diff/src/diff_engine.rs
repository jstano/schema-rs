use schema_model::model::column::Column;
use schema_model::model::constraint::Constraint;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::schema::Schema;
use schema_model::model::types::KeyType;
use schema_model::model::view::View;

use crate::change::SchemaChange;
use crate::change_set::ChangeSet;

pub struct SchemaDiffEngine;

impl SchemaDiffEngine {
    pub fn diff(old: &Schema, new: &Schema) -> ChangeSet {
        let mut change_set = ChangeSet::new();

        // Drop phase (order matters: views → relations → keys → constraints → columns → tables)
        diff_drop_views(old, new, &mut change_set);
        diff_drop_relations(old, new, &mut change_set);
        diff_drop_keys(old, new, &mut change_set);
        diff_drop_constraints(old, new, &mut change_set);
        diff_drop_columns(old, new, &mut change_set);
        diff_drop_tables(old, new, &mut change_set);

        // Add phase (order matters: tables → columns → modify columns → keys → constraints → relations → views)
        diff_add_tables(old, new, &mut change_set);
        diff_add_columns(old, new, &mut change_set);
        diff_modify_columns(old, new, &mut change_set);
        diff_add_keys(old, new, &mut change_set);
        diff_add_constraints(old, new, &mut change_set);
        diff_add_relations(old, new, &mut change_set);
        diff_add_views(old, new, &mut change_set);

        change_set
    }
}

fn diff_drop_tables(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if new.get_optional_table(old_table.name()).is_none() {
            cs.add_change(SchemaChange::DropTable {
                table_name: old_table.name().to_string(),
            });
        }
    }
}

fn diff_add_tables(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if old.get_optional_table(new_table.name()).is_none() {
            cs.add_change(SchemaChange::AddTable {
                table_name: new_table.name().to_string(),
            });
        }
    }
}

fn diff_drop_columns(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if let Some(new_table) = new.get_optional_table(old_table.name()) {
            for old_col in old_table.columns() {
                if !new_table.has_column(old_col.name()) {
                    let rename_candidates: Vec<String> = new_table
                        .columns()
                        .iter()
                        .filter(|nc| !old_table.has_column(nc.name()) && nc.column_type() == old_col.column_type())
                        .map(|nc| nc.name().to_string())
                        .collect();
                    cs.add_change(SchemaChange::DropColumn {
                        table_name: old_table.name().to_string(),
                        column_name: old_col.name().to_string(),
                        rename_candidates,
                    });
                }
            }
        }
    }
}

fn diff_add_columns(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            for new_col in new_table.columns() {
                if !old_table.has_column(new_col.name()) {
                    cs.add_change(SchemaChange::AddColumn {
                        table_name: new_table.name().to_string(),
                        column: new_col.clone(),
                    });
                }
            }
        }
    }
}

fn diff_modify_columns(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            for new_col in new_table.columns() {
                if old_table.has_column(new_col.name()) {
                    let old_col = old_table.column(new_col.name());
                    if columns_differ(old_col, new_col) {
                        cs.add_change(SchemaChange::ModifyColumn {
                            table_name: new_table.name().to_string(),
                            old_column: old_col.clone(),
                            new_column: new_col.clone(),
                        });
                    }
                }
            }
        }
    }
}

fn columns_differ(a: &Column, b: &Column) -> bool {
    a.column_type() != b.column_type()
        || a.length() != b.length()
        || a.scale() != b.scale()
        || a.required() != b.required()
        || a.default_constraint() != b.default_constraint()
        || a.check_constraint() != b.check_constraint()
        || a.enum_type() != b.enum_type()
        || a.element_type() != b.element_type()
        || a.generated() != b.generated()
        || a.min_value() != b.min_value()
        || a.max_value() != b.max_value()
}

fn diff_drop_keys(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if let Some(new_table) = new.get_optional_table(old_table.name()) {
            // Ordinal counts only siblings of the same category (unique keys among unique
            // keys, indexes among indexes) in list order - matching how the create path
            // numbers `ak_<table><n>` / `ix_<table><n>` - so the migration generator can
            // reproduce the exact same name for the object it's dropping.
            let mut unique_ordinal = 0;
            for old_key in old_table.keys() {
                if old_key.key_type() == KeyType::Unique {
                    unique_ordinal += 1;
                }
                if !key_exists_in(old_key, new_table.keys()) && !key_exists_in(old_key, new_table.indexes()) {
                    cs.add_change(SchemaChange::DropKey {
                        table_name: old_table.name().to_string(),
                        key: old_key.clone(),
                        ordinal: unique_ordinal,
                    });
                }
            }

            let mut index_ordinal = 0;
            for old_key in old_table.indexes() {
                if old_key.is_index() {
                    index_ordinal += 1;
                }
                if !key_exists_in(old_key, new_table.keys()) && !key_exists_in(old_key, new_table.indexes()) {
                    cs.add_change(SchemaChange::DropKey {
                        table_name: old_table.name().to_string(),
                        key: old_key.clone(),
                        ordinal: index_ordinal,
                    });
                }
            }
        }
    }
}

fn diff_add_keys(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            // See diff_drop_keys - same per-category ordinal so an added key gets the
            // name the create path would give it.
            let mut unique_ordinal = 0;
            for new_key in new_table.keys() {
                if new_key.key_type() == KeyType::Unique {
                    unique_ordinal += 1;
                }
                if !key_exists_in(new_key, old_table.keys()) && !key_exists_in(new_key, old_table.indexes()) {
                    cs.add_change(SchemaChange::AddKey {
                        table_name: new_table.name().to_string(),
                        key: new_key.clone(),
                        ordinal: unique_ordinal,
                    });
                }
            }

            let mut index_ordinal = 0;
            for new_key in new_table.indexes() {
                if new_key.is_index() {
                    index_ordinal += 1;
                }
                if !key_exists_in(new_key, old_table.keys()) && !key_exists_in(new_key, old_table.indexes()) {
                    cs.add_change(SchemaChange::AddKey {
                        table_name: new_table.name().to_string(),
                        key: new_key.clone(),
                        ordinal: index_ordinal,
                    });
                }
            }
        }
    }
}

fn key_exists_in(key: &Key, keys: &[Key]) -> bool {
    keys.iter().any(|k| keys_equal(k, key))
}

fn keys_equal(a: &Key, b: &Key) -> bool {
    a.key_type() == b.key_type()
        && a.is_unique() == b.is_unique()
        && a.is_cluster() == b.is_cluster()
        && a.include() == b.include()
        && a.columns().len() == b.columns().len()
        && a.columns().iter().zip(b.columns().iter()).all(|(ac, bc)| {
            ac.name().eq_ignore_ascii_case(bc.name())
        })
}

fn diff_drop_constraints(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if let Some(new_table) = new.get_optional_table(old_table.name()) {
            for old_con in old_table.constraints() {
                if !constraint_exists_in(old_con, new_table.constraints()) {
                    cs.add_change(SchemaChange::DropConstraint {
                        table_name: old_table.name().to_string(),
                        constraint_name: old_con.name().to_string(),
                    });
                }
            }
        }
    }
}

fn diff_add_constraints(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            for new_con in new_table.constraints() {
                if !constraint_exists_in(new_con, old_table.constraints()) {
                    cs.add_change(SchemaChange::AddConstraint {
                        table_name: new_table.name().to_string(),
                        constraint: new_con.clone(),
                    });
                }
            }
        }
    }
}

fn constraint_exists_in(con: &Constraint, constraints: &[Constraint]) -> bool {
    constraints.iter().any(|c| constraints_equal(c, con))
}

fn constraints_equal(a: &Constraint, b: &Constraint) -> bool {
    a.name().eq_ignore_ascii_case(b.name()) && a.sql().trim() == b.sql().trim()
}

fn diff_drop_relations(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if let Some(new_table) = new.get_optional_table(old_table.name()) {
            // Ordinal is the relation's 1-based position in the table's relation list,
            // matching the create path's `fk_<table><n>` numbering, so the migration
            // generator can reproduce the exact name of the constraint it's dropping.
            for (index, old_rel) in old_table.relations().iter().enumerate() {
                if !relation_exists_in(old_rel, new_table.relations()) {
                    cs.add_change(SchemaChange::DropRelation {
                        relation: old_rel.clone(),
                        ordinal: index + 1,
                    });
                }
            }
        }
    }
}

fn diff_add_relations(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            // See diff_drop_relations - same ordinal so an added relation gets the name
            // the create path would give it.
            for (index, new_rel) in new_table.relations().iter().enumerate() {
                if !relation_exists_in(new_rel, old_table.relations()) {
                    cs.add_change(SchemaChange::AddRelation {
                        relation: new_rel.clone(),
                        ordinal: index + 1,
                    });
                }
            }
        }
    }
}

fn relation_exists_in(rel: &Relation, relations: &[Relation]) -> bool {
    relations.iter().any(|r| relations_equal(r, rel))
}

fn relations_equal(a: &Relation, b: &Relation) -> bool {
    a.from_table_name().eq_ignore_ascii_case(b.from_table_name())
        && a.from_column_name().eq_ignore_ascii_case(b.from_column_name())
        && a.to_table_name().eq_ignore_ascii_case(b.to_table_name())
        && a.to_column_name().eq_ignore_ascii_case(b.to_column_name())
        && a.relation_type() == b.relation_type()
}

fn diff_drop_views(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_view in old.all_views() {
        if !view_exists_in(old_view, new.all_views()) {
            cs.add_change(SchemaChange::DropView {
                view_name: old_view.name().to_string(),
            });
        }
    }
}

fn diff_add_views(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_view in new.all_views() {
        if !view_exists_in(new_view, old.all_views()) {
            cs.add_change(SchemaChange::AddView {
                view: new_view.clone(),
            });
        }
    }
}

fn view_exists_in(view: &View, views: &[View]) -> bool {
    views.iter().any(|v| views_equal(v, view))
}

fn views_equal(a: &View, b: &View) -> bool {
    a.name().eq_ignore_ascii_case(b.name()) && a.sql().trim() == b.sql().trim()
}
