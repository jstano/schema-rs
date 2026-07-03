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

impl TriggerGenerator for PostgresTriggerGenerator {
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

impl PostgresTriggerGenerator {
    fn output_delete_trigger(&self, table: &Table, pk_col: &str, separator: &str) {
        let table_name = table.name().to_lowercase();
        let fn_name = format!("{}_delete", table_name);
        let fully_qualified_table = table.fully_qualified_table_name();
        let fully_qualified_fn = format!(
            "{}.{}",
            table.schema_name().unwrap_or("public"),
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
                for relation in table.reverse_relations() {
                    match relation.relation_type() {
                        RelationType::Enforce => {
                            let to_table = self.database_model().find_table(
                                relation.to_table_name().split('.').next(),
                                relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                            );
                            sql_println!(
                                writer,
                                "   if (select count(*) from {} where {} = OLD.{}) > 0 then",
                                to_table.fully_qualified_table_name(),
                                relation.to_column_name(),
                                pk_col
                            );
                            sql_println!(
                                writer,
                                "      raise exception 'The row in {} cannot be deleted. It is being used by a row in the {} table.';",
                                fully_qualified_table,
                                to_table.fully_qualified_table_name()
                            );
                            sql_println!(writer, "   end if;");
                        }
                        RelationType::SetNull => {
                            let to_table = self.database_model().find_table(
                                relation.to_table_name().split('.').next(),
                                relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                            );
                            sql_println!(
                                writer,
                                "   update {} set {} = null where {} = OLD.{};",
                                to_table.fully_qualified_table_name(),
                                relation.to_column_name(),
                                relation.to_column_name(),
                                pk_col
                            );
                        }
                        RelationType::Cascade => {
                            let to_table = self.database_model().find_table(
                                relation.to_table_name().split('.').next(),
                                relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                            );
                            sql_println!(
                                writer,
                                "   delete from {} where {} = OLD.{};",
                                to_table.fully_qualified_table_name(),
                                relation.to_column_name(),
                                pk_col
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
        let table_name = table.name().to_lowercase();
        let fn_name = format!("{}_update", table_name);
        let fully_qualified_table = table.fully_qualified_table_name();
        let fully_qualified_fn = format!(
            "{}.{}",
            table.schema_name().unwrap_or("public"),
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
                            let to_table = self.database_model().find_table(
                                relation.to_table_name().split('.').next(),
                                relation.to_table_name().split('.').last().unwrap_or(&relation.to_table_name()),
                            );
                            sql_println!(
                                writer,
                                "   if new.{} is not null then",
                                relation.from_column_name()
                            );
                            sql_println!(
                                writer,
                                "      if (select count(*) from {} where {} = new.{}) = 0 then",
                                to_table.fully_qualified_table_name(),
                                relation.to_column_name(),
                                relation.from_column_name()
                            );
                            sql_println!(
                                writer,
                                "         raise exception 'The value of {} was not found in the {} table.';",
                                relation.from_column_name(),
                                to_table.fully_qualified_table_name()
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
