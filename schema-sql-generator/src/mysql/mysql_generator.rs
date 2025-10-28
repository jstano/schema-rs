use crate::common::function_generator::DefaultFunctionGenerator;
use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::DefaultIndexGenerator;
use crate::common::other_sql_generator::DefaultOtherSqlGenerator;
use crate::common::procedure_generator::DefaultProcedureGenerator;
use crate::common::relation_generator::DefaultRelationGenerator;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::common::trigger_generator::DefaultTriggerGenerator;
use crate::common::view_generator::DefaultViewGenerator;
use crate::mysql::mysql_table_generator::MySqlTableGenerator;

pub struct MySqlGenerator {
    context: GeneratorContext,
    sql_generator: DefaultSqlGenerator,
}

impl MySqlGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(MySqlTableGenerator::new(context.clone())),
            Box::new(DefaultRelationGenerator::new(context.clone())),
            Box::new(DefaultIndexGenerator::new(context.clone())),
            Box::new(DefaultFunctionGenerator::new(context.clone())),
            Box::new(DefaultViewGenerator::new(context.clone())),
            Box::new(DefaultProcedureGenerator::new(context.clone())),
            Box::new(DefaultTriggerGenerator::new(context.clone())),
            Box::new(DefaultOtherSqlGenerator::new(context.clone())),
        );

        Self {
            context,
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
