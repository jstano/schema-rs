use crate::common::generator_context::GeneratorContext;
use crate::common::table_generator::{DefaultTableGenerator, TableGenerator};
use crate::sqlite::sqlite_column_constraint_generator::SqliteColumnConstraintGenerator;
use crate::sqlite::sqlite_column_generator::SqliteColumnGenerator;
use crate::sqlite::sqlite_index_generator::SqliteIndexGenerator;
use crate::sqlite::sqlite_key_generator::SqliteKeyGenerator;
use crate::sqlite::sqlite_relation_generator::SqliteRelationGenerator;
use crate::sqlite::sqlite_table_constraint_generator::SqliteTableConstraintGenerator;
use schema_model::model::table::Table;

pub struct SqliteTableGenerator {
    context: GeneratorContext,
    table_generator: DefaultTableGenerator,
    relation_generator: SqliteRelationGenerator,
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
            relation_generator: SqliteRelationGenerator::new(context.clone()),
            context,
        }
    }
}

impl TableGenerator for SqliteTableGenerator {
    fn output_tables(&self) {
        // Self-dispatching loop (rather than delegating to
        // `DefaultTableGenerator::output_tables`) so that `self.output_table_definition` below
        // resolves to *this* impl's override chain instead of statically binding to
        // `DefaultTableGenerator::output_table_definition` - see the `SqlServerTableGenerator`
        // comment for the same pattern. Without this, `output_table_definition`'s inline
        // foreign-key handling is unreachable and SQLite output silently has no foreign keys.
        //
        // All drops are emitted up front, in reverse-dependency order, rather than
        // interleaved with each table's own create (H17): SQLite has no `cascade` on
        // `drop table`, so dropping in declaration order can fail on the first table
        // that's still an FK target of a later one. See `tables_in_drop_order`.
        let database_model = self.context.settings().database_model();

        for table in crate::common::table_generator::tables_in_drop_order(database_model) {
            self.output_table_drop(table);
        }

        for schema in database_model.schemas() {
            for table in schema.tables() {
                self.output_table_header(table);
                self.output_table_definition(table);
                self.output_table_footer(table);
                self.output_indexes(table);
                self.output_initial_data(table);
            }
        }
    }

    fn output_table(&self, table: &Table) {
        self.output_table_drop(table);
        self.output_table_header(table);
        self.output_table_definition(table);
        self.output_table_footer(table);
        self.output_indexes(table);
        self.output_initial_data(table);
    }

    fn output_table_drop(&self, table: &Table) {
        self.table_generator.output_table_drop(table);
    }

    fn output_table_header(&self, table: &Table) {
        self.table_generator.output_table_header(table);
    }

    fn output_table_definition(&self, table: &Table) {
        // SQLite has no `ALTER TABLE ... ADD CONSTRAINT` support, so foreign keys
        // must be declared inline in the `CREATE TABLE` statement rather than as
        // separate statements from the relation generator.
        let inline_foreign_keys = self.relation_generator.inline_foreign_key_constraints(table);
        self.table_generator.output_table_definition_with_extra(table, inline_foreign_keys);
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
    use schema_model::model::key::{Key, KeyColumn};
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode, KeyType};

    #[test]
    fn output_table_renders_header_and_columns() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "name", ColumnType::Varchar).length(50).required(true).build())
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteTableGenerator::new(ctx);
        generator.output_table_header(&table);
        generator.output_table_definition(&table);
        generator.output_table_footer(&table);

        let output = buffer.contents();
        assert!(output.contains("create table users"));
        assert!(output.contains("id integer primary key autoincrement"));
        // The table-level `constraint pk_... primary key (...)` must not also be emitted -
        // SQLite rejects a table with more than one primary key declaration.
        assert!(!output.contains("primary key (id)"));
        assert!(output.contains("name varchar(50)"));
        assert!(output.contains(");"));
    }

    #[test]
    fn output_tables_reaches_inline_foreign_keys_via_self_dispatch() {
        // Regression test for H1: `output_tables` must call back into *this* type's
        // `output_table_definition` override (not `DefaultTableGenerator`'s), or SQLite
        // output silently has no foreign keys under `--foreign-key-mode relations`.
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(schema_model::model::relation::Relation::new(
                "parent", "id", "child", "parent_id", schema_model::model::types::RelationType::Cascade, false,
            ))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteTableGenerator::new(ctx);
        generator.output_tables();

        let output = buffer.contents();
        assert!(output.contains("foreign key (parent_id) references parent(id)"));
    }

    #[test]
    fn output_tables_drops_child_before_parent_and_before_any_create() {
        // Regression test for H17: dropping in declaration order (parent first, since
        // it must be declared before the child that references it) fails on a rerun
        // against a populated database - the child row referencing "parent" still
        // exists when "drop table parent" runs. All drops must be emitted up front, in
        // reverse-dependency order (child before parent), ahead of every create.
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(Key::new(KeyType::Primary, vec![KeyColumn::new("id")]))
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_relation(schema_model::model::relation::Relation::new(
                "parent", "id", "child", "parent_id", schema_model::model::types::RelationType::Cascade, false,
            ))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = SqliteTableGenerator::new(ctx);
        generator.output_tables();

        let output = buffer.contents();
        let drop_child = output.find("drop table if exists child").unwrap();
        let drop_parent = output.find("drop table if exists parent").unwrap();
        let create_child = output.find("create table child").unwrap();
        let create_parent = output.find("create table parent").unwrap();

        assert!(drop_child < drop_parent, "child must be dropped before the parent it references");
        assert!(drop_parent < create_child, "every drop must precede every create");
        assert!(drop_parent < create_parent, "every drop must precede every create");
    }
}
