use schema_model::model::column::Column;
use schema_model::model::constraint::Constraint;
use schema_model::model::enum_type::EnumType;
use schema_model::model::function::Function;
use schema_model::model::initial_data::InitialData;
use schema_model::model::key::Key;
use schema_model::model::other_sql::OtherSql;
use schema_model::model::procedure::Procedure;
use schema_model::model::relation::Relation;
use schema_model::model::schema::Schema;
use schema_model::model::trigger::Trigger;
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
        diff_drop_triggers(old, new, &mut change_set);
        diff_drop_functions(old, new, &mut change_set);
        diff_drop_procedures(old, new, &mut change_set);
        diff_drop_other_sql(old, new, &mut change_set);
        diff_drop_initial_data(old, new, &mut change_set);
        // Last: Postgres can't DROP TYPE while a column still references it.
        diff_drop_enum_types(old, new, &mut change_set);

        // Add phase (order matters: tables → columns → modify columns → keys → constraints → relations → views)
        // Enum types/functions/procedures/other_sql go first - columns and tables may depend on them.
        // Also detects modified enum types (same name, different value list).
        diff_add_enum_types(old, new, &mut change_set);
        diff_add_functions(old, new, &mut change_set);
        diff_add_procedures(old, new, &mut change_set);
        diff_add_other_sql(old, new, &mut change_set);
        diff_add_tables(old, new, &mut change_set);
        diff_add_columns(old, new, &mut change_set);
        diff_modify_columns(old, new, &mut change_set);
        diff_add_keys(old, new, &mut change_set);
        diff_add_constraints(old, new, &mut change_set);
        diff_add_relations(old, new, &mut change_set);
        diff_add_views(old, new, &mut change_set);
        diff_add_triggers(old, new, &mut change_set);
        // Last: initial data rows reference columns/keys that must already exist.
        diff_add_initial_data(old, new, &mut change_set);

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
        && a.filter() == b.filter()
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
        && a.to_table_name().eq_ignore_ascii_case(b.to_table_name())
        && a.relation_type() == b.relation_type()
        && column_pairs_equal(a.column_pairs(), b.column_pairs())
}

fn column_pairs_equal(a: &[(String, String)], b: &[(String, String)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|((af, at), (bf, bt))| {
            af.eq_ignore_ascii_case(bf) && at.eq_ignore_ascii_case(bt)
        })
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

fn diff_add_enum_types(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_enum in new.enum_types() {
        match find_enum_type(old, new_enum.name()) {
            None => cs.add_change(SchemaChange::AddEnumType { enum_type: new_enum.clone() }),
            Some(old_enum) if !enum_types_equal(old_enum, new_enum) => {
                cs.add_change(SchemaChange::ModifyEnumType {
                    old_enum_type: old_enum.clone(),
                    new_enum_type: new_enum.clone(),
                });
            }
            Some(_) => {}
        }
    }
}

fn diff_drop_enum_types(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_enum in old.enum_types() {
        if find_enum_type(new, old_enum.name()).is_none() {
            cs.add_change(SchemaChange::DropEnumType {
                enum_type_name: old_enum.name().to_string(),
            });
        }
    }
}

fn find_enum_type<'a>(schema: &'a Schema, name: &str) -> Option<&'a EnumType> {
    schema.enum_types().find(|e| e.name().eq_ignore_ascii_case(name))
}

fn enum_types_equal(a: &EnumType, b: &EnumType) -> bool {
    a.name().eq_ignore_ascii_case(b.name())
        && a.values().len() == b.values().len()
        && a.values()
            .iter()
            .zip(b.values().iter())
            .all(|(av, bv)| av.name() == bv.name() && av.code() == bv.code())
}

fn diff_add_functions(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_fn in new.functions() {
        if !functions_contains(old.functions(), new_fn) {
            cs.add_change(SchemaChange::AddFunction { function: new_fn.clone() });
        }
    }
}

fn diff_drop_functions(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_fn in old.functions() {
        if !functions_contains(new.functions(), old_fn) {
            cs.add_change(SchemaChange::DropFunction {
                function_name: old_fn.name().to_string(),
                database_type: old_fn.database_type(),
            });
        }
    }
}

fn functions_contains(functions: &[Function], target: &Function) -> bool {
    functions.iter().any(|f| functions_equal(f, target))
}

fn functions_equal(a: &Function, b: &Function) -> bool {
    a.name().eq_ignore_ascii_case(b.name())
        && a.database_type() == b.database_type()
        && a.sql().trim() == b.sql().trim()
}

fn diff_add_procedures(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_proc in new.procedures() {
        if !procedures_contains(old.procedures(), new_proc) {
            cs.add_change(SchemaChange::AddProcedure { procedure: new_proc.clone() });
        }
    }
}

fn diff_drop_procedures(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_proc in old.procedures() {
        if !procedures_contains(new.procedures(), old_proc) {
            cs.add_change(SchemaChange::DropProcedure {
                procedure_name: old_proc.name().to_string(),
                database_type: old_proc.database_type(),
            });
        }
    }
}

fn procedures_contains(procedures: &[Procedure], target: &Procedure) -> bool {
    procedures.iter().any(|p| procedures_equal(p, target))
}

fn procedures_equal(a: &Procedure, b: &Procedure) -> bool {
    a.name().eq_ignore_ascii_case(b.name())
        && a.database_type() == b.database_type()
        && a.sql().trim() == b.sql().trim()
}

fn diff_add_other_sql(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_sql in new.other_sql() {
        if !other_sql_contains(old.other_sql(), new_sql) {
            cs.add_change(SchemaChange::AddOtherSql { other_sql: new_sql.clone() });
        }
    }
}

fn diff_drop_other_sql(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_sql in old.other_sql() {
        if !other_sql_contains(new.other_sql(), old_sql) {
            cs.add_change(SchemaChange::DropOtherSql { other_sql: old_sql.clone() });
        }
    }
}

fn other_sql_contains(entries: &[OtherSql], target: &OtherSql) -> bool {
    entries.iter().any(|o| other_sql_equal(o, target))
}

fn other_sql_equal(a: &OtherSql, b: &OtherSql) -> bool {
    a.database_type() == b.database_type() && a.order() == b.order() && a.sql().trim() == b.sql().trim()
}

// Triggers and initial data are table-scoped, and - like `diff_add_columns`/`diff_drop_columns`
// - only diffed for tables present in both old and new. A brand-new table's columns aren't
// emitted as `AddColumn`s either (see those functions), so a brand-new table's triggers/initial
// data have the same pre-existing gap; not fixed here.
fn diff_add_triggers(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            for new_trigger in new_table.triggers() {
                if !triggers_contains(old_table.triggers(), new_trigger) {
                    cs.add_change(SchemaChange::AddTrigger {
                        table_name: new_table.name().to_string(),
                        trigger: new_trigger.clone(),
                    });
                }
            }
        }
    }
}

fn diff_drop_triggers(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if let Some(new_table) = new.get_optional_table(old_table.name()) {
            for old_trigger in old_table.triggers() {
                if !triggers_contains(new_table.triggers(), old_trigger) {
                    cs.add_change(SchemaChange::DropTrigger {
                        table_name: old_table.name().to_string(),
                        trigger: old_trigger.clone(),
                    });
                }
            }
        }
    }
}

fn triggers_contains(triggers: &[Trigger], target: &Trigger) -> bool {
    triggers.iter().any(|t| triggers_equal(t, target))
}

fn triggers_equal(a: &Trigger, b: &Trigger) -> bool {
    a.trigger_text().trim() == b.trigger_text().trim()
        && a.trigger_type() == b.trigger_type()
        && a.database_type() == b.database_type()
}

fn diff_add_initial_data(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for new_table in new.tables() {
        if let Some(old_table) = old.get_optional_table(new_table.name()) {
            for new_data in new_table.initial_data() {
                if !initial_data_contains(old_table.initial_data(), new_data) {
                    cs.add_change(SchemaChange::AddInitialData {
                        table_name: new_table.name().to_string(),
                        initial_data: new_data.clone(),
                    });
                }
            }
        }
    }
}

fn diff_drop_initial_data(old: &Schema, new: &Schema, cs: &mut ChangeSet) {
    for old_table in old.tables() {
        if let Some(new_table) = new.get_optional_table(old_table.name()) {
            for old_data in old_table.initial_data() {
                if !initial_data_contains(new_table.initial_data(), old_data) {
                    cs.add_change(SchemaChange::DropInitialData {
                        table_name: old_table.name().to_string(),
                        initial_data: old_data.clone(),
                    });
                }
            }
        }
    }
}

fn initial_data_contains(entries: &[InitialData], target: &InitialData) -> bool {
    entries.iter().any(|d| initial_data_equal(d, target))
}

fn initial_data_equal(a: &InitialData, b: &InitialData) -> bool {
    a.sql().trim() == b.sql().trim() && a.database_type() == b.database_type()
}
