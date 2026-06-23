use crate::common::generator_context::GeneratorContext;
use crate::common::trigger_generator::TriggerGenerator;
use crate::sql_println;
use schema_model::model::table::Table;
use schema_model::model::types::{ForeignKeyMode, RelationType, TriggerType, DatabaseType};

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

    fn get_primary_key_column(&self, table: &Table) -> Option<String> {
        table
            .primary_key_columns()
            .and_then(|mut cols| cols.pop())
            .and_then(|col_name| {
                if table.has_column(&col_name) {
                    Some(col_name)
                } else {
                    None
                }
            })
    }
}

impl TriggerGenerator for SqlServerTriggerGenerator {
    fn output_triggers(&self) {
        let database_model = self.context.settings().database_model();
        let separator = self.context.settings().statement_separator();

        for table in database_model.all_tables() {
            if self.should_output_delete_trigger(table) {
                if let Some(pk_col) = self.get_primary_key_column(table) {
                    self.output_delete_trigger(table, &pk_col, separator);
                }
            }

            if self.should_output_update_trigger(table) {
                self.output_update_trigger(table, separator);
            }
        }
    }
}

impl SqlServerTriggerGenerator {
    fn output_delete_trigger(&self, table: &Table, pk_col: &str, separator: &str) {
        let table_name = table.name().to_lowercase();
        let fully_qualified_table = table.fully_qualified_table_name();

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {}_delete */", table_name);
            sql_println!(
                writer,
                "if exists (select name from dbo.sysobjects where name = '{}_delete' and type = 'TR')",
                table_name
            );
            sql_println!(writer, "   drop trigger {}_delete{}", table_name, separator);
            sql_println!(writer, "");
            sql_println!(writer, "create trigger {}_delete on {} for delete as", table_name, fully_qualified_table);
            sql_println!(writer, "if (select count(*) from deleted) > 0");
            sql_println!(writer, "BEGIN");

            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers {
                let mut first_enforce = true;
                for relation in table.reverse_relations() {
                    if matches!(relation.relation_type(), RelationType::Enforce) {
                        if first_enforce {
                            sql_println!(writer, "   declare @msg varchar(2000)");
                            first_enforce = false;
                        }
                        let to_table = self.database_model().find_table(
                            relation.to_table_name().split('.').next(),
                            relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                        );
                        sql_println!(
                            writer,
                            "   if (select count(*) from {} where {} in (select {} from deleted)) > 0",
                            to_table.fully_qualified_table_name(),
                            relation.to_column_name(),
                            pk_col
                        );
                        sql_println!(writer, "   begin");
                        sql_println!(
                            writer,
                            "      select @msg = 'The {} ' + (select top 1 convert(varchar, {}) from deleted where {} in (select {} from {})) + ' cannot be deleted. It is being used by a row in the {} table.'",
                            fully_qualified_table,
                            pk_col,
                            pk_col,
                            relation.to_column_name(),
                            to_table.fully_qualified_table_name(),
                            to_table.fully_qualified_table_name()
                        );
                        sql_println!(writer, "      rollback transaction");
                        sql_println!(writer, "      raiserror (@msg, 16, 1)");
                        sql_println!(writer, "      return");
                        sql_println!(writer, "   end;");
                    }
                }

                for relation in table.reverse_relations() {
                    if matches!(relation.relation_type(), RelationType::SetNull) {
                        let to_table = self.database_model().find_table(
                            relation.to_table_name().split('.').next(),
                            relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                        );
                        sql_println!(
                            writer,
                            "   update {} set {} = null where {} in (select {} from deleted);",
                            to_table.fully_qualified_table_name(),
                            relation.to_column_name(),
                            relation.to_column_name(),
                            pk_col
                        );
                    }
                }

                for relation in table.reverse_relations() {
                    if matches!(relation.relation_type(), RelationType::Cascade) {
                        let to_table = self.database_model().find_table(
                            relation.to_table_name().split('.').next(),
                            relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                        );
                        sql_println!(
                            writer,
                            "   delete from {} where {} in (select {} from deleted);",
                            to_table.fully_qualified_table_name(),
                            relation.to_column_name(),
                            pk_col
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
        let table_name = table.name().to_lowercase();
        let fully_qualified_table = table.fully_qualified_table_name();

        self.context.with_writer(|writer| {
            sql_println!(writer, "/* {}_update */", table_name);
            sql_println!(
                writer,
                "if exists (select name from dbo.sysobjects where name = '{}_update' and type = 'TR')",
                table_name
            );
            sql_println!(writer, "   drop trigger {}_update{}", table_name, separator);
            sql_println!(writer, "");
            sql_println!(writer, "create trigger {}_update on {} for insert, update as", table_name, fully_qualified_table);
            sql_println!(writer, "if (select count(*) from inserted) > 0");
            sql_println!(writer, "BEGIN");

            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Triggers {
                for relation in table.relations() {
                    match relation.relation_type() {
                        RelationType::Enforce | RelationType::SetNull | RelationType::Cascade => {
                            let to_table = self.database_model().find_table(
                                relation.to_table_name().split('.').next(),
                                relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                            );
                            sql_println!(
                                writer,
                                "   if (select count(*) from inserted where {} is not null and {} not in (select {} from {})) > 0",
                                relation.from_column_name(),
                                relation.from_column_name(),
                                relation.to_column_name(),
                                to_table.fully_qualified_table_name()
                            );
                            sql_println!(writer, "   begin");
                            sql_println!(
                                writer,
                                "      raiserror ('The value of {} was not found in the {} table.', 16, 1)",
                                relation.from_column_name(),
                                to_table.fully_qualified_table_name()
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
