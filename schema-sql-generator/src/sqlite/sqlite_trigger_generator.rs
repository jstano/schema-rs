use crate::common::generator_context::GeneratorContext;
use crate::common::trigger_generator::{DefaultTriggerGenerator, TriggerGenerator};

pub struct SqliteTriggerGenerator {
    trigger_generator: DefaultTriggerGenerator,
}

impl SqliteTriggerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            trigger_generator: DefaultTriggerGenerator::new(context),
        }
    }
}

impl TriggerGenerator for SqliteTriggerGenerator {
    fn output_triggers(&self) {
        self.trigger_generator.output_triggers();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_triggers_is_a_no_op_for_sqlite() {
        // SQLite has no trigger support in this generator; DefaultTriggerGenerator is
        // intentionally a no-op here (unlike postgres/sqlserver which override it).
        let table = TableBuilder::new(None::<&str>, "users").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Triggers, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteTriggerGenerator::new(ctx);
        generator.output_triggers();

        assert_eq!(buffer.contents(), "");
    }
}
