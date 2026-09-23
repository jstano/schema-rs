use crate::common::generator_context::GeneratorContext;
use schema_model::model::table::Table;

pub trait TriggerGenerator {
    fn output_triggers(&self);

    /// Regenerates the full (idempotent) trigger set for a single table - used by the
    /// migration generator when only that table's custom triggers changed, so it doesn't
    /// have to re-derive the combined relation/aggregation/custom-trigger logic that
    /// `output_triggers` already knows how to produce. No-op by default (SQLite has no
    /// trigger support here, matching `output_triggers`).
    fn output_triggers_for_table(&self, table: &Table);
}

pub struct DefaultTriggerGenerator {
    context: GeneratorContext,
}

impl DefaultTriggerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            context,
        }
    }

    pub fn context(&self) -> &GeneratorContext {
        &self.context
    }
}

impl TriggerGenerator for DefaultTriggerGenerator {
    fn output_triggers(&self) {
    }

    fn output_triggers_for_table(&self, _table: &Table) {
    }
}