use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::sqlserver::sqlserver_function_generator::SqlServerFunctionGenerator;
use crate::sqlserver::sqlserver_index_generator::SqlServerIndexGenerator;
use crate::sqlserver::sqlserver_other_sql_generator::SqlServerOtherSqlGenerator;
use crate::sqlserver::sqlserver_procedure_generator::SqlServerProcedureGenerator;
use crate::sqlserver::sqlserver_relation_generator::SqlServerRelationGenerator;
use crate::sqlserver::sqlserver_table_generator::SqlServerTableGenerator;
use crate::sqlserver::sqlserver_trigger_generator::SqlServerTriggerGenerator;
use crate::sqlserver::sqlserver_view_generator::SqlServerViewGenerator;

pub struct SqlServerGenerator {
    sql_generator: DefaultSqlGenerator,
}

impl SqlServerGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(SqlServerTableGenerator::new(context.clone())),
            Box::new(SqlServerRelationGenerator::new(context.clone())),
            Box::new(SqlServerIndexGenerator::new(context.clone())),
            Box::new(SqlServerFunctionGenerator::new(context.clone())),
            Box::new(SqlServerViewGenerator::new(context.clone())),
            Box::new(SqlServerProcedureGenerator::new(context.clone())),
            Box::new(SqlServerTriggerGenerator::new(context.clone())),
            Box::new(SqlServerOtherSqlGenerator::new(context.clone())),
        );

        Self {
            sql_generator,
        }
    }
}

impl SqlGenerator for SqlServerGenerator {
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
