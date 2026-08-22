use crate::common::generator_context::GeneratorContext;
use crate::common::sql_string::escape_sql_literal;
use crate::common::trigger_generator::TriggerGenerator;
use crate::sql_println;
use schema_model::model::table::Table;
use schema_model::model::types::{DatabaseType, ForeignKeyMode, RelationType, TriggerType};

pub struct SqlServerTriggerGenerator {
    context: GeneratorContext,
}

impl SqlServerTriggerGenerator {
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

impl TriggerGenerator for SqlServerTriggerGenerator {
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

impl SqlServerTriggerGenerator {
    fn output_delete_trigger(&self, table: &Table, separator: &str) {
        let database_type = self.context.settings().database_type();
        let table_name = table.name().to_lowercase();
        let fully_qualified_table = table.fully_qualified_table_name(database_type);
        let fully_qualified_trigger = database_type.qualified_name(table.schema_name(), &format!("{}_delete", table_name));

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {}_delete */", table_name);
            sql_println!(
                writer,
                "if exists (select name from dbo.sysobjects where name = '{}_delete' and type = 'TR')",
                escape_sql_literal(&table_name)
            );
            sql_println!(writer, "   drop trigger {}{}", fully_qualified_trigger, separator);
            sql_println!(writer, "");
            sql_println!(writer, "create trigger {}_delete on {} for delete as", table_name, fully_qualified_table);
            sql_println!(writer, "if (select count(*) from deleted) > 0");
            sql_println!(writer, "BEGIN");

            // A reverse relation carries the *original* relation's fields unchanged
            // (attached to the parent for convenient lookup): `from_table_name`/
            // `from_column_name` identify the child table and its FK column, while
            // `to_table_name`/`to_column_name` still refer to this table (the parent)
            // itself. The child table must be resolved via `from_table_name`, not
            // `to_table_name` - using `to_table_name` here would resolve back to this
            // same table and generate a trigger that deletes/updates/checks itself.
            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers {
                let mut first_enforce = true;
                for relation in table.reverse_relations() {
                    if matches!(relation.relation_type(), RelationType::Enforce) {
                        if first_enforce {
                            sql_println!(writer, "   declare @msg varchar(2000)");
                            first_enforce = false;
                        }
                        let child_table = self.database_model().find_table_by_qualified_name(relation.from_table_name());
                        sql_println!(
                            writer,
                            "   if (select count(*) from {} where {} in (select {} from deleted)) > 0",
                            child_table.fully_qualified_table_name(database_type),
                            relation.from_column_name(),
                            relation.to_column_name()
                        );
                        sql_println!(writer, "   begin");
                        sql_println!(
                            writer,
                            "      select @msg = 'The {} ' + (select top 1 convert(varchar, {}) from deleted where {} in (select {} from {})) + ' cannot be deleted. It is being used by a row in the {} table.'",
                            fully_qualified_table,
                            relation.to_column_name(),
                            relation.to_column_name(),
                            relation.from_column_name(),
                            child_table.fully_qualified_table_name(database_type),
                            child_table.fully_qualified_table_name(database_type)
                        );
                        sql_println!(writer, "      rollback transaction");
                        sql_println!(writer, "      raiserror (@msg, 16, 1)");
                        sql_println!(writer, "      return");
                        sql_println!(writer, "   end;");
                    }
                }

                for relation in table.reverse_relations() {
                    if matches!(relation.relation_type(), RelationType::SetNull) {
                        let child_table = self.database_model().find_table_by_qualified_name(relation.from_table_name());
                        sql_println!(
                            writer,
                            "   update {} set {} = null where {} in (select {} from deleted);",
                            child_table.fully_qualified_table_name(database_type),
                            relation.from_column_name(),
                            relation.from_column_name(),
                            relation.to_column_name()
                        );
                    }
                }

                for relation in table.reverse_relations() {
                    if matches!(relation.relation_type(), RelationType::Cascade) {
                        let child_table = self.database_model().find_table_by_qualified_name(relation.from_table_name());
                        sql_println!(
                            writer,
                            "   delete from {} where {} in (select {} from deleted);",
                            child_table.fully_qualified_table_name(database_type),
                            relation.from_column_name(),
                            relation.to_column_name()
                        );
                    }
                }
            }

            for custom_trigger in table.triggers() {
                if custom_trigger.trigger_type() == TriggerType::Delete
                    && custom_trigger.database_type() == DatabaseType::SqlServer
                {
                    sql_println!(writer, "{}", custom_trigger.trigger_text());
                }
            }

            sql_println!(writer, "END{}", separator);
            sql_println!(writer, "");
        });
    }

    fn output_update_trigger(&self, table: &Table, separator: &str) {
        let database_type = self.context.settings().database_type();
        let table_name = table.name().to_lowercase();
        let fully_qualified_table = table.fully_qualified_table_name(database_type);
        let fully_qualified_trigger = database_type.qualified_name(table.schema_name(), &format!("{}_update", table_name));

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {}_update */", table_name);
            sql_println!(
                writer,
                "if exists (select name from dbo.sysobjects where name = '{}_update' and type = 'TR')",
                escape_sql_literal(&table_name)
            );
            sql_println!(writer, "   drop trigger {}{}", fully_qualified_trigger, separator);
            sql_println!(writer, "");
            sql_println!(writer, "create trigger {}_update on {} for insert, update as", table_name, fully_qualified_table);
            sql_println!(writer, "if (select count(*) from inserted) > 0");
            sql_println!(writer, "BEGIN");

            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers {
                for relation in table.relations() {
                    match relation.relation_type() {
                        RelationType::Enforce | RelationType::SetNull | RelationType::Cascade => {
                            let to_table = self.database_model().find_table_by_qualified_name(relation.to_table_name());
                            sql_println!(
                                writer,
                                "   if (select count(*) from inserted where {} is not null and {} not in (select {} from {})) > 0",
                                relation.from_column_name(),
                                relation.from_column_name(),
                                relation.to_column_name(),
                                to_table.fully_qualified_table_name(database_type)
                            );
                            sql_println!(writer, "   begin");
                            sql_println!(
                                writer,
                                "      raiserror ('The value of {} was not found in the {} table.', 16, 1)",
                                relation.from_column_name(),
                                to_table.fully_qualified_table_name(database_type)
                            );
                            sql_println!(writer, "      rollback transaction");
                            sql_println!(writer, "      return");
                            sql_println!(writer, "   end;");
                        }
                        RelationType::DoNothing => {}
                    }
                }
            }

            for custom_trigger in table.triggers() {
                if custom_trigger.trigger_type() == TriggerType::Update
                    && custom_trigger.database_type() == DatabaseType::SqlServer
                {
                    sql_println!(writer, "{}", custom_trigger.trigger_text());
                }
            }

            sql_println!(writer, "END{}", separator);
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
        // Schema-qualify both the tables and the relation's to_table_name: find_table() here
        // does relation.to_table_name().split('.').next() to get a schema name, which only
        // works for qualified references (mirrors the same quirk in PostgresTriggerGenerator).
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

    #[test]
    fn output_triggers_renders_update_validation_trigger_for_relations_mode_triggers() {
        let model = build_model_with_relation();
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Triggers);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("create trigger child_update on app.child for insert, update as"));
        assert!(output.contains("was not found in the app.parent table"));
        assert!(output.contains("raiserror"));
        assert!(output.contains("rollback transaction"));
    }

    #[test]
    fn output_triggers_qualifies_drop_trigger_with_schema() {
        let model = build_model_with_relation();
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Triggers);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("drop trigger app.child_update"));
    }

    #[test]
    fn output_triggers_qualifies_drop_trigger_with_default_schema_when_none() {
        let table = TableBuilder::new(None::<&str>, "widget")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_trigger(schema_model::model::trigger::Trigger::new(
                "-- custom",
                schema_model::model::types::TriggerType::Update,
                DatabaseType::SqlServer,
            ))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Relations);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("drop trigger dbo.widget_update"));
    }

    #[test]
    fn output_triggers_escapes_single_quote_in_table_name() {
        // Regression test: an unescaped embedded quote would break the generated
        // sysobjects existence-check SQL string literal.
        let table = TableBuilder::new(None::<&str>, "o'brien")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_trigger(schema_model::model::trigger::Trigger::new(
                "-- custom",
                schema_model::model::types::TriggerType::Update,
                DatabaseType::SqlServer,
            ))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Relations);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("where name = 'o''brien_update' and type = 'TR'"));
    }

    #[test]
    fn output_triggers_does_nothing_when_relations_mode_is_not_triggers() {
        let model = build_model_with_relation();
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Relations);

        let generator = SqlServerTriggerGenerator::new(ctx);
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
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Triggers);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("create trigger parent_delete on app.parent for delete as"));
        assert!(output.contains("if (select count(*) from app.child where parent_id in (select id from deleted)) > 0"));
        assert!(output.contains("cannot be deleted. It is being used by a row in the app.child table"));
    }

    #[test]
    fn output_delete_trigger_setnull_updates_the_child_table_not_itself() {
        let model = build_model_with_reverse_relation(RelationType::SetNull);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Triggers);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("update app.child set parent_id = null where parent_id in (select id from deleted);"));
    }

    #[test]
    fn output_delete_trigger_cascade_deletes_from_the_child_table_not_itself() {
        let model = build_model_with_reverse_relation(RelationType::Cascade);
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Triggers);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("delete from app.child where parent_id in (select id from deleted);"));
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
        let (ctx, buffer) = make_context_with_fk_mode(model, DatabaseType::SqlServer, ForeignKeyMode::Triggers);

        let generator = SqlServerTriggerGenerator::new(ctx);
        generator.output_triggers();

        let output = buffer.contents();
        assert!(output.contains("delete from app.child where parent_id in (select id from deleted);"));
    }
}
