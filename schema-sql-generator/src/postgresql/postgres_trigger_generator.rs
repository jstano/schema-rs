use crate::common::generator_context::GeneratorContext;
use crate::common::trigger_generator::TriggerGenerator;
use crate::sql_println;
use schema_model::model::table::Table;
use schema_model::model::types::{DatabaseType, ForeignKeyMode, RelationType, TriggerType};

pub struct PostgresTriggerGenerator {
    context: GeneratorContext,
}

impl PostgresTriggerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self { context }
    }

    fn should_output_delete_trigger(&self, table: &Table) -> bool {
        let has_delete_triggers = table
            .triggers()
            .iter()
            .any(|t| t.trigger_type() == TriggerType::Delete);

        let has_reverse_relations_with_triggers = !table.reverse_relations().is_empty()
            && self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers;

        let has_aggregations = !table.aggregations().is_empty();

        has_delete_triggers || has_reverse_relations_with_triggers || has_aggregations
    }

    fn should_output_update_trigger(&self, table: &Table) -> bool {
        let has_update_triggers = table
            .triggers()
            .iter()
            .any(|t| t.trigger_type() == TriggerType::Update);

        let has_relations_with_triggers = !table.relations().is_empty()
            && self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers;

        let has_aggregations = !table.aggregations().is_empty();

        has_update_triggers || has_relations_with_triggers || has_aggregations
    }

}

impl TriggerGenerator for PostgresTriggerGenerator {
    fn output_triggers(&self) {
        let database_model = self.context.settings().database_model();
        let separator = self.context.settings().statement_separator();

        for table in database_model.all_tables() {
            if self.should_output_delete_trigger(table) && table.primary_key().is_some() {
                self.output_delete_trigger(table, separator);
            }

            if self.should_output_update_trigger(table) {
                self.output_update_trigger(table, separator);
            }
        }
    }
}

impl PostgresTriggerGenerator {
    fn output_delete_trigger(&self, table: &Table, separator: &str) {
        let database_type = self.context.settings().database_type();
        let table_name = table.name().to_lowercase();
        let fn_name = format!("{}_delete", table_name);
        let fully_qualified_table = table.fully_qualified_table_name(database_type);
        let fully_qualified_fn = format!(
            "{}.{}",
            table.schema_name().unwrap_or(DatabaseType::Postgresql.default_schema().unwrap()),
            fn_name
        );

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {} */", fully_qualified_fn);
            sql_println!(
                writer,
                "create or replace function {}() returns trigger as $BODY$"
            ,
                fully_qualified_fn
            );
            sql_println!(writer, "begin");

            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers {
                // A reverse relation carries the *original* relation's fields unchanged
                // (attached to the parent for convenient lookup): `from_table_name`/
                // `from_column_name` identify the child table and its FK column, while
                // `to_table_name`/`to_column_name` still refer to this table (the parent)
                // itself. The child table must be resolved via `from_table_name`, not
                // `to_table_name` - using `to_table_name` here would resolve back to this
                // same table and generate a trigger that deletes/updates/checks itself.
                for relation in table.reverse_relations() {
                    match relation.relation_type() {
                        RelationType::Enforce => {
                            let child_table = self.database_model().find_table_by_qualified_name(relation.from_table_name());
                            sql_println!(
                                writer,
                                "   if (select count(*) from {} where {} = OLD.{}) > 0 then",
                                child_table.fully_qualified_table_name(database_type),
                                relation.from_column_name(),
                                relation.to_column_name()
                            );
                            sql_println!(
                                writer,
                                "      raise exception 'The row in {} cannot be deleted. It is being used by a row in the {} table.';",
                                fully_qualified_table,
                                child_table.fully_qualified_table_name(database_type)
                            );
                            sql_println!(writer, "   end if;");
                        }
                        RelationType::SetNull => {
                            let child_table = self.database_model().find_table_by_qualified_name(relation.from_table_name());
                            sql_println!(
                                writer,
                                "   update {} set {} = null where {} = OLD.{};",
                                child_table.fully_qualified_table_name(database_type),
                                relation.from_column_name(),
                                relation.from_column_name(),
                                relation.to_column_name()
                            );
                        }
                        RelationType::Cascade => {
                            let child_table = self.database_model().find_table_by_qualified_name(relation.from_table_name());
                            sql_println!(
                                writer,
                                "   delete from {} where {} = OLD.{};",
                                child_table.fully_qualified_table_name(database_type),
                                relation.from_column_name(),
                                relation.to_column_name()
                            );
                        }
                        RelationType::DoNothing => {}
                    }
                }
            }

            for custom_trigger in table.triggers() {
                if custom_trigger.trigger_type() == TriggerType::Delete
                    && custom_trigger.database_type() == DatabaseType::Postgresql
                {
                    sql_println!(writer, "{}", custom_trigger.trigger_text());
                }
            }

            sql_println!(writer, "   return null;");
            sql_println!(writer, "end;");
            sql_println!(writer, "$BODY$ language plpgsql{}", separator);
            sql_println!(writer, "");

            sql_println!(writer, "drop trigger if exists {} on {} cascade{}", table_name, fully_qualified_table, separator);
            sql_println!(
                writer,
                "create trigger {} after delete on {}",
                table_name,
                fully_qualified_table
            );
            sql_println!(writer, "   for each row execute procedure {}(){}", fully_qualified_fn, separator);
            sql_println!(writer, "");
        });
    }

    fn output_update_trigger(&self, table: &Table, separator: &str) {
        let database_type = self.context.settings().database_type();
        let table_name = table.name().to_lowercase();
        let fn_name = format!("{}_update", table_name);
        let fully_qualified_table = table.fully_qualified_table_name(database_type);
        let fully_qualified_fn = format!(
            "{}.{}",
            table.schema_name().unwrap_or(DatabaseType::Postgresql.default_schema().unwrap()),
            fn_name
        );

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {} */", fully_qualified_fn);
            sql_println!(
                writer,
                "create or replace function {}() returns trigger as $BODY$"
            ,
                fully_qualified_fn
            );
            sql_println!(writer, "begin");

            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers {
                for relation in table.relations() {
                    match relation.relation_type() {
                        RelationType::Enforce | RelationType::SetNull | RelationType::Cascade => {
                            let to_table = self.database_model().find_table_by_qualified_name(relation.to_table_name());
                            sql_println!(
                                writer,
                                "   if new.{} is not null then",
                                relation.from_column_name()
                            );
                            sql_println!(
                                writer,
                                "      if (select count(*) from {} where {} = new.{}) = 0 then",
                                to_table.fully_qualified_table_name(database_type),
                                relation.to_column_name(),
                                relation.from_column_name()
                            );
                            sql_println!(
                                writer,
                                "         raise exception 'The value of {} was not found in the {} table.';",
                                relation.from_column_name(),
                                to_table.fully_qualified_table_name(database_type)
                            );
                            sql_println!(writer, "      end if;");
                            sql_println!(writer, "   end if;");
                        }
                        RelationType::DoNothing => {}
                    }
                }
            }

            for custom_trigger in table.triggers() {
                if custom_trigger.trigger_type() == TriggerType::Update
                    && custom_trigger.database_type() == DatabaseType::Postgresql
                {
                    sql_println!(writer, "{}", custom_trigger.trigger_text());
                }
            }

            sql_println!(writer, "   return new;");
            sql_println!(writer, "end;");
            sql_println!(writer, "$BODY$ language plpgsql{}", separator);
            sql_println!(writer, "");

            sql_println!(writer, "drop trigger if exists {} on {} cascade{}", table_name, fully_qualified_table, separator);
            sql_println!(
                writer,
                "create trigger {} after insert or update on {}",
                table_name,
                fully_qualified_table
            );
            sql_println!(writer, "   for each row execute procedure {}(){}", fully_qualified_fn, separator);
            sql_println!(writer, "");
        });
    }

    fn database_model(&self) -> &schema_model::model::database_model::DatabaseModel {
        self.context.settings().database_model()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context_with_fk_mode;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::relation::Relation;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, RelationType};

    fn build_model_with_relation() -> DatabaseModel {
        let parent = TableBuilder::new(Some("app"), "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(Some("app"), "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("app.parent", "id", "child", "parent_id", RelationType::Enforce, false))
            .build();
        let schema = SchemaBuilder::new(Some("app"))
            .add_table(parent)
            .add_table(child)
            .build();
        DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
    }

    fn build_model_with_unqualified_relation() -> DatabaseModel {
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::Enforce, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .build();
        DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
    }

    #[test]
    fn output_triggers_renders_update_validation_trigger_for_relations_mode_triggers() {
        let model = build_model_with_relation();
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Triggers);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("create or replace function app.child_update() returns trigger"));
        assert!(output.contains("was not found in the app.parent table"));
        assert!(output.contains("create trigger child after insert or update on app.child"));
    }

    #[test]
    fn output_triggers_resolves_unqualified_to_table_name_in_default_schema() {
        let model = build_model_with_unqualified_relation();
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Triggers);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("was not found in the public.parent table"));
    }

    #[test]
    fn output_triggers_does_nothing_when_relations_mode_is_not_triggers() {
        let model = build_model_with_relation();
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Relations);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        assert_eq!(buffer.contents(), "");
    }

    /// Builds parent/child tables with both the forward relation (on `child`, used for the
    /// update trigger) and the matching reverse relation attached to `parent` (used for the
    /// delete trigger) - mirroring what `convert::reverse_relations()` does for a real parsed
    /// schema, since these unit tests build the model directly via builders rather than XML.
    fn build_model_with_reverse_relation(relation_type: RelationType) -> DatabaseModel {
        let mut parent = TableBuilder::new(Some("app"), "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(schema_model::builder::KeyBuilder::new(schema_model::model::types::KeyType::Primary).add_column("id").build())
            .build();
        parent.add_reverse_relation(Relation::new("app.parent", "id", "app.child", "parent_id", relation_type, false));

        let child = TableBuilder::new(Some("app"), "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("app.parent", "id", "child", "parent_id", relation_type, false))
            .build();
        let schema = SchemaBuilder::new(Some("app"))
            .add_table(parent)
            .add_table(child)
            .build();
        DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema])
    }

    #[test]
    fn output_delete_trigger_enforce_checks_the_child_table_not_itself() {
        let model = build_model_with_reverse_relation(RelationType::Enforce);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Triggers);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("create or replace function app.parent_delete"));
        assert!(output.contains("if (select count(*) from app.child where parent_id = OLD.id) > 0"));
        assert!(output.contains("cannot be deleted. It is being used by a row in the app.child table"));
    }

    #[test]
    fn output_delete_trigger_setnull_updates_the_child_table_not_itself() {
        let model = build_model_with_reverse_relation(RelationType::SetNull);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Triggers);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("update app.child set parent_id = null where parent_id = OLD.id;"));
    }

    #[test]
    fn output_delete_trigger_cascade_deletes_from_the_child_table_not_itself() {
        let model = build_model_with_reverse_relation(RelationType::Cascade);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Triggers);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("delete from app.child where parent_id = OLD.id;"));
    }

    #[test]
    fn output_delete_trigger_uses_the_relations_own_column_for_a_composite_primary_key() {
        // The parent has a composite primary key (id, tenant_id); the relation specifically
        // references `id`. The delete trigger must match on the relation's own referenced
        // column, not an arbitrarily-picked primary key column.
        let mut parent = TableBuilder::new(Some("app"), "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "tenant_id", ColumnType::Int).required(true).build())
            .add_key(
                schema_model::builder::KeyBuilder::new(schema_model::model::types::KeyType::Primary)
                    .add_column("id")
                    .add_column("tenant_id")
                    .build(),
            )
            .build();
        parent.add_reverse_relation(Relation::new("app.parent", "id", "app.child", "parent_id", RelationType::Cascade, false));

        let child = TableBuilder::new(Some("app"), "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .build();
        let schema = SchemaBuilder::new(Some("app"))
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::Postgresql, ForeignKeyMode::Triggers);

        let generator = PostgresTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("delete from app.child where parent_id = OLD.id;"));
    }
}


