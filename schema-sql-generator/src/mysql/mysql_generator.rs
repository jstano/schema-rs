use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::mysql::mysql_function_generator::MySqlFunctionGenerator;
use crate::mysql::mysql_index_generator::MySqlIndexGenerator;
use crate::mysql::mysql_other_sql_generator::MySqlOtherSqlGenerator;
use crate::mysql::mysql_procedure_generator::MySqlProcedureGenerator;
use crate::mysql::mysql_relation_generator::MySqlRelationGenerator;
use crate::mysql::mysql_table_generator::MySqlTableGenerator;
use crate::mysql::mysql_trigger_generator::MySqlTriggerGenerator;
use crate::mysql::mysql_view_generator::MySqlViewGenerator;

pub struct MySqlGenerator {
    sql_generator: DefaultSqlGenerator,
}

impl MySqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(MySqlTableGenerator::new(context.clone())),
            Box::new(MySqlRelationGenerator::new(context.clone())),
            Box::new(MySqlIndexGenerator::new(context.clone())),
            Box::new(MySqlFunctionGenerator::new(context.clone())),
            Box::new(MySqlViewGenerator::new(context.clone())),
            Box::new(MySqlProcedureGenerator::new(context.clone())),
            Box::new(MySqlTriggerGenerator::new(context.clone())),
            Box::new(MySqlOtherSqlGenerator::new(context.clone())),
        );

        Self {
            sql_generator,
        }
    }
}

impl SqlGenerator for MySqlGenerator {
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
