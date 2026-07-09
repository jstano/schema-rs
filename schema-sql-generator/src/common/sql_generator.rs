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
    fn context(&self) -> &GeneratorContext;

    fn generate(&self)  {
        self.output_sql();
    }

    fn output_sql(&self) {
        self.output_header();

        if self.context().settings().output_mode() == OutputMode::IndexesOnly {
            self.output_indexes();
        } else if self.context().settings().output_mode() == OutputMode::TriggersOnly {
            self.output_triggers();
        } else {
            self.output_other_sql_top();
            self.output_tables();

            if self.context().settings().foreign_key_mode() == ForeignKeyMode::Relations {
                self.output_relations();
            }

            self.output_triggers();
            self.output_functions();
            self.output_views();
            self.output_procedures();
            self.output_other_sql_bottom();
        }
    }

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
    fn context(&self) -> &GeneratorContext {
        &self.context
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::function_generator::FunctionGenerator;
    use crate::common::generate_options::GenerateOptions;
    use crate::common::index_generator::IndexGenerator;
    use crate::common::other_sql_generator::OtherSqlGenerator;
    use crate::common::print_writer::PrintWriter;
    use crate::common::procedure_generator::ProcedureGenerator;
    use crate::common::relation_generator::RelationGenerator;
    use crate::common::sql_generator_settings::SqlGeneratorSettings;
    use crate::common::sql_writer::SqlWriter;
    use crate::common::table_generator::TableGenerator;
    use crate::common::trigger_generator::TriggerGenerator;
    use crate::common::view_generator::ViewGenerator;
    use schema_model::builder::SchemaBuilder;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::key::Key;
    use schema_model::model::table::Table;
    use crate::common::output_mode::OutputMode;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct CallLog(Rc<RefCell<Vec<&'static str>>>);

    impl CallLog {
        fn record(&self, name: &'static str) {
            self.0.borrow_mut().push(name);
        }

        fn calls(&self) -> Vec<&'static str> {
            self.0.borrow().clone()
        }
    }

    struct FakeTableGenerator(CallLog);
    impl TableGenerator for FakeTableGenerator {
        fn output_tables(&self) { self.0.record("tables"); }
        fn output_table(&self, _table: &Table) {}
        fn output_table_header(&self, _table: &Table) {}
        fn output_table_definition(&self, _table: &Table) {}
        fn output_table_footer(&self, _table: &Table) {}
        fn output_indexes(&self, _table: &Table) {}
        fn output_initial_data(&self, _table: &Table) {}
    }

    struct FakeRelationGenerator(CallLog);
    impl RelationGenerator for FakeRelationGenerator {
        fn output_relations(&self) { self.0.record("relations"); }
    }

    struct FakeIndexGenerator(CallLog);
    impl IndexGenerator for FakeIndexGenerator {
        fn output_indexes(&self) { self.0.record("indexes"); }
        fn output_indexes_for_table(&self, _writer: &mut SqlWriter, _table: &Table) {}
        fn output_index(&self, _writer: &mut SqlWriter, _statement_separator: &str, _table: &Table, _key_name: &str, _key: &Key) {}
        fn index_options(&self, _key: &Key) -> Option<String> { None }
    }

    struct FakeFunctionGenerator(CallLog);
    impl FunctionGenerator for FakeFunctionGenerator {
        fn output_functions(&self) { self.0.record("functions"); }
        fn output_function(&self, _writer: &mut SqlWriter, _statement_separator: &str, _function: &schema_model::model::function::Function) {}
    }

    struct FakeViewGenerator(CallLog);
    impl ViewGenerator for FakeViewGenerator {
        fn output_views(&self) { self.0.record("views"); }
    }

    struct FakeProcedureGenerator(CallLog);
    impl ProcedureGenerator for FakeProcedureGenerator {
        fn output_procedures(&self) { self.0.record("procedures"); }
        fn output_procedure(&self, _writer: &mut SqlWriter, _statement_separator: &str, _procedure: &schema_model::model::procedure::Procedure) {}
    }

    struct FakeTriggerGenerator(CallLog);
    impl TriggerGenerator for FakeTriggerGenerator {
        fn output_triggers(&self) { self.0.record("triggers"); }
    }

    struct FakeOtherSqlGenerator(CallLog);
    impl OtherSqlGenerator for FakeOtherSqlGenerator {
        fn output_other_sql_top(&self) { self.0.record("other_sql_top"); }
        fn output_other_sql_bottom(&self) { self.0.record("other_sql_bottom"); }
        fn output_other_sql(&self, _writer: &mut SqlWriter, _statement_separator: &str, _sql: &str) {}
    }

    fn make_generator(
        foreign_key_mode: ForeignKeyMode,
        output_mode: OutputMode,
    ) -> (DefaultSqlGenerator, CallLog) {
        let log = CallLog::default();
        let schema = SchemaBuilder::new(None::<&str>).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let mut options = GenerateOptions::new(
            Rc::new(model),
            Rc::new(RefCell::new(PrintWriter::new(Box::new(Vec::<u8>::new())))),
        );
        options.foreign_key_mode = foreign_key_mode;
        options.output_mode = output_mode;
        let settings = SqlGeneratorSettings::new(DatabaseType::Postgresql, &options);
        let writer = SqlWriter::new(options.writer.clone());
        let context = GeneratorContext::new(settings, writer);

        let generator = DefaultSqlGenerator::new(
            context,
            Box::new(FakeTableGenerator(log.clone())),
            Box::new(FakeRelationGenerator(log.clone())),
            Box::new(FakeIndexGenerator(log.clone())),
            Box::new(FakeFunctionGenerator(log.clone())),
            Box::new(FakeViewGenerator(log.clone())),
            Box::new(FakeProcedureGenerator(log.clone())),
            Box::new(FakeTriggerGenerator(log.clone())),
            Box::new(FakeOtherSqlGenerator(log.clone())),
        );
        (generator, log)
    }

    #[test]
    fn output_sql_runs_full_pipeline_in_order_when_foreign_key_mode_is_relations() {
        let (generator, log) = make_generator(ForeignKeyMode::Relations, OutputMode::All);
        generator.output_sql();

        assert_eq!(
            log.calls(),
            vec!["other_sql_top", "tables", "relations", "triggers", "functions", "views", "procedures", "other_sql_bottom"]
        );
    }

    #[test]
    fn output_sql_skips_relations_when_foreign_key_mode_is_not_relations() {
        let (generator, log) = make_generator(ForeignKeyMode::Triggers, OutputMode::All);
        generator.output_sql();

        assert_eq!(
            log.calls(),
            vec!["other_sql_top", "tables", "triggers", "functions", "views", "procedures", "other_sql_bottom"]
        );
        assert!(!log.calls().contains(&"relations"));
    }

    #[test]
    fn output_sql_only_runs_indexes_when_output_mode_is_indexes_only() {
        let (generator, log) = make_generator(ForeignKeyMode::Relations, OutputMode::IndexesOnly);
        generator.output_sql();

        assert_eq!(log.calls(), vec!["indexes"]);
    }

    #[test]
    fn output_sql_only_runs_triggers_when_output_mode_is_triggers_only() {
        let (generator, log) = make_generator(ForeignKeyMode::Relations, OutputMode::TriggersOnly);
        generator.output_sql();

        assert_eq!(log.calls(), vec!["triggers"]);
    }
}
