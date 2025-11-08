use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::h2::h2_function_generator::H2FunctionGenerator;
use crate::h2::h2_index_generator::H2IndexGenerator;
use crate::h2::h2_other_sql_generator::H2OtherSqlGenerator;
use crate::h2::h2_procedure_generator::H2ProcedureGenerator;
use crate::h2::h2_relation_generator::H2RelationGenerator;
use crate::h2::h2_table_generator::H2TableGenerator;
use crate::h2::h2_trigger_generator::H2TriggerGenerator;
use crate::h2::h2_view_generator::H2ViewGenerator;

pub struct H2Generator {
    sql_generator: DefaultSqlGenerator,
}

impl H2Generator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(H2TableGenerator::new(context.clone())),
            Box::new(H2RelationGenerator::new(context.clone())),
            Box::new(H2IndexGenerator::new(context.clone())),
            Box::new(H2FunctionGenerator::new(context.clone())),
            Box::new(H2ViewGenerator::new(context.clone())),
            Box::new(H2ProcedureGenerator::new(context.clone())),
            Box::new(H2TriggerGenerator::new(context.clone())),
            Box::new(H2OtherSqlGenerator::new(context.clone())),
        );

        Self {
            sql_generator,
        }
    }
}

impl SqlGenerator for H2Generator {
    fn context(&self) -> &GeneratorContext {
        self.sql_generator.context()
    }

    fn generate(&self) {
        self.sql_generator.generate()
    }

    fn output_sql(&self) {
        self.sql_generator.output_sql();
    }

    fn output_header(&self) {
        self.sql_generator.output_header();
    }

    fn output_tables(&self) {
        self.sql_generator.output_tables();
    }

    fn output_relations(&self) {
        self.sql_generator.output_relations();
    }

    fn output_indexes(&self) {
        self.sql_generator.output_indexes();
    }

    fn output_triggers(&self) {
        self.sql_generator.output_triggers();
    }

    fn output_functions(&self) {
        self.sql_generator.output_functions();
    }

    fn output_views(&self) {
        self.sql_generator.output_views();
    }

    fn output_procedures(&self) {
        self.sql_generator.output_procedures();
    }

    fn output_other_sql_top(&self) {
        self.sql_generator.output_other_sql_top();
    }

    fn output_other_sql_bottom(&self) {
        self.sql_generator.output_other_sql_bottom();
    }
}
