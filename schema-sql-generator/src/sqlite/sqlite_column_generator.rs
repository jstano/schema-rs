use crate::common::column_generator::{ColumnGenerator, DefaultColumnGenerator, DefaultConstraintNaming};
use crate::common::generator_context::GeneratorContext;
use crate::sqlite::sqlite_column_type_generator::SqliteColumnTypeGenerator;
use crate::sqlite::sqlite_pk_support::inline_autoincrement_pk_column;
use schema_model::model::column::Column;
use schema_model::model::table::Table;

pub struct SqliteColumnGenerator {
    column_generator: DefaultColumnGenerator,
}

impl SqliteColumnGenerator {
    pub fn new(context: GeneratorContext) -> Self {
        Self {
            column_generator: DefaultColumnGenerator::new(
                context.clone(),
                Box::new(SqliteColumnTypeGenerator::new(context.clone())),
                DefaultConstraintNaming::NamedByColumn,
            ),
        }
    }
}

impl ColumnGenerator for SqliteColumnGenerator {
    fn column_definitions(&self, table: &Table) -> Vec<String> {
        // Self-dispatching (not a delegation to `DefaultColumnGenerator::column_definitions`)
        // so each column routes through *this* type's `column_sql` override below - the same
        // static-dispatch trap as `SqliteTableGenerator`/`SqlServerTableGenerator::output_tables`.
        // Delegating here would silently skip the inline-autoincrement primary key handling.
        table.columns().iter().map(|column| self.column_sql(table, column)).collect()
    }

    fn column_sql(&self, table: &Table, column: &Column) -> String {
        // A Sequence/LongSequence column that is the table's sole primary key column must be
        // declared `integer primary key autoincrement` inline - see `sqlite_pk_support`. Any
        // `not null`/default options are skipped: they're redundant (PRIMARY KEY already
        // implies NOT NULL) and the corresponding table-level PRIMARY KEY constraint is
        // suppressed by `SqliteKeyGenerator` to avoid SQLite's "more than one primary key"
        // error.
        if inline_autoincrement_pk_column(table).is_some_and(|pk_column| pk_column.name().eq_ignore_ascii_case(column.name())) {
            return format!("   {} integer primary key autoincrement", column.name());
        }

        self.column_generator.column_sql(table, column)
    }

    fn column_options(&self, table: &Table, column: &Column) -> String {
        self.column_generator.column_options(table, column)
    }

    fn default_value(&self, table: &Table, column: &Column) -> Option<String> {
        self.column_generator.default_value(table, column)
    }
}
