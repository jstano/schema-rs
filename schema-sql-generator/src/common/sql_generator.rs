use crate::common::function_generator::FunctionGenerator;
use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::IndexGenerator;
use crate::common::other_sql_generator::OtherSqlGenerator;
use crate::common::output_mode::OutputMode;
use crate::common::procedure_generator::ProcedureGenerator;
use crate::common::relation_generator::RelationGenerator;
use crate::common::table_generator::TableGenerator;
use crate::common::trigger_generator::TriggerGenerator;
use crate::common::view_generator::ViewGenerator;
use schema_model::model::types::ForeignKeyMode;

pub trait SqlGenerator {
    fn generate(&self);
    fn output_sql(&self);
    fn output_header(&self);
    fn output_tables(&self);
    fn output_relations(&self);
    fn output_indexes(&self);
    fn output_triggers(&self);
    fn output_functions(&self);
    fn output_views(&self);
    fn output_procedures(&self);
    fn output_other_sql_top(&self);
    fn output_other_sql_bottom(&self);
}

pub struct DefaultSqlGenerator {
    context: GeneratorContext,
    table_generator: Box<dyn TableGenerator>,
    relation_generator: Box<dyn RelationGenerator>,
    index_generator: Box<dyn IndexGenerator>,
    function_generator: Box<dyn FunctionGenerator>,
    view_generator: Box<dyn ViewGenerator>,
    procedure_generator: Box<dyn ProcedureGenerator>,
    trigger_generator: Box<dyn TriggerGenerator>,
    other_sql_generator: Box<dyn OtherSqlGenerator>,
}

impl DefaultSqlGenerator {
    pub fn new(
        context: GeneratorContext,
        table_generator: Box<dyn TableGenerator>,
        relation_generator: Box<dyn RelationGenerator>,
        index_generator: Box<dyn IndexGenerator>,
        function_generator: Box<dyn FunctionGenerator>,
        view_generator: Box<dyn ViewGenerator>,
        procedure_generator: Box<dyn ProcedureGenerator>,
        trigger_generator: Box<dyn TriggerGenerator>,
        other_sql_generator: Box<dyn OtherSqlGenerator>,
    ) -> Self {
        Self {
            context,
            table_generator,
            relation_generator,
            index_generator,
            function_generator,
            view_generator,
            procedure_generator,
            trigger_generator,
            other_sql_generator,
        }
    }
}

impl SqlGenerator for DefaultSqlGenerator {
    fn generate(&self) {
        self.output_sql();
    }

    fn output_sql(&self) {
        self.output_header();

        if self.context.settings().output_mode() == OutputMode::IndexesOnly {
            self.output_indexes();
        } else if self.context.settings().output_mode() == OutputMode::TriggersOnly {
            self.output_triggers();
        } else {
            self.output_other_sql_top();
            self.output_tables();

            if self.context.settings().foreign_key_mode() == ForeignKeyMode::Relations {
                self.output_relations();
            }

            self.output_triggers();
            self.output_functions();
            self.output_views();
            self.output_procedures();
            self.output_other_sql_bottom();
        }
    }

    fn output_header(&self) {
    }

    fn output_tables(&self) {
        self.table_generator.output_tables();
    }

    fn output_relations(&self) {
        self.relation_generator.output_relations();
    }

    fn output_indexes(&self) {
        self.index_generator.output_indexes();
    }

    fn output_triggers(&self) {
        self.trigger_generator.output_triggers();
    }

    fn output_functions(&self) {
        self.function_generator.output_functions();
    }

    fn output_views(&self) {
        self.view_generator.output_views();
    }

    fn output_procedures(&self) {
        self.procedure_generator.output_procedures();
    }

    fn output_other_sql_top(&self) {
        self.other_sql_generator.output_other_sql_top();
    }

    fn output_other_sql_bottom(&self) {
        self.other_sql_generator.output_other_sql_bottom();
    }
}
