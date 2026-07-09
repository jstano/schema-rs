use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::sqlite::sqlite_column_constraint_generator::SqliteColumnConstraintGenerator;
use crate::sqlite::sqlite_column_generator::SqliteColumnGenerator;
use crate::sqlite::sqlite_index_generator::SqliteIndexGenerator;
use crate::sqlite::sqlite_key_generator::SqliteKeyGenerator;
use crate::sqlite::sqlite_table_constraint_generator::SqliteTableConstraintGenerator;
use schema_model::model::table::Table;

pub struct SqliteTableGenerator {
    table_generator: DefaultTableGenerator,
}

impl SqliteTableGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            table_generator: DefaultTableGenerator::new(
                context.clone(),
                Box::new(SqliteColumnGenerator::new(context.clone())),
                Box::new(SqliteKeyGenerator::new(context.clone())),
                Box::new(SqliteColumnConstraintGenerator::new(context.clone())),
                Box::new(SqliteTableConstraintGenerator::new(context.clone())),
                Box::new(SqliteIndexGenerator::new(context.clone())),
            ),
        }
    }
}

impl TableGenerator for SqliteTableGenerator {
    fn output_tables(&self) {
        self.table_generator.output_tables();
    }

    fn output_table(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_header(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_definition(&self, table: &Table) {
        self.table_generator.output_table_definition(table);
    }

    fn output_table_footer(&self, table: &Table) {
        self.table_generator.output_table_footer(table);
    }

    fn output_indexes(&self, table: &Table) {
        self.table_generator.output_indexes(table);
    }

    fn output_initial_data(&self, table: &Table) {
        self.table_generator.output_initial_data(table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    #[test]
    fn output_table_renders_header_and_columns() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "name", ColumnType::Varchar).length(50).required(true).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(None, BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteTableGenerator::new(ctx);
        generator.output_table_header(&table);
        generator.output_table_definition(&table);
        generator.output_table_footer(&table);

        let output = buffer.contents();
        assert!(output.contains("create table users"));
        assert!(output.contains("id integer auto_increment"));
        assert!(output.contains("name varchar(50)"));
        assert!(output.contains(");"));
    }
}
