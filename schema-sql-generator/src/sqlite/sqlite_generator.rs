use crate::common::generator_context::GeneratorContext;
use crate::common::sql_generator::{DefaultSqlGenerator, SqlGenerator};
use crate::sqlite::sqlite_function_generator::SqliteFunctionGenerator;
use crate::sqlite::sqlite_index_generator::SqliteIndexGenerator;
use crate::sqlite::sqlite_other_sql_generator::SqliteOtherSqlGenerator;
use crate::sqlite::sqlite_procedure_generator::SqliteProcedureGenerator;
use crate::sqlite::sqlite_relation_generator::SqliteRelationGenerator;
use crate::sqlite::sqlite_table_generator::SqliteTableGenerator;
use crate::sqlite::sqlite_trigger_generator::SqliteTriggerGenerator;
use crate::sqlite::sqlite_view_generator::SqliteViewGenerator;

pub struct SqliteGenerator {
    sql_generator: DefaultSqlGenerator,
}

impl SqliteGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        let sql_generator = DefaultSqlGenerator::new(
            context.clone(),
            Box::new(SqliteTableGenerator::new(context.clone())),
            Box::new(SqliteRelationGenerator::new(context.clone())),
            Box::new(SqliteIndexGenerator::new(context.clone())),
            Box::new(SqliteFunctionGenerator::new(context.clone())),
            Box::new(SqliteViewGenerator::new(context.clone())),
            Box::new(SqliteProcedureGenerator::new(context.clone())),
            Box::new(SqliteTriggerGenerator::new(context.clone())),
            Box::new(SqliteOtherSqlGenerator::new(context.clone())),
        );

        Self {
            sql_generator,
        }
    }
}

impl SqlGenerator for SqliteGenerator {
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
