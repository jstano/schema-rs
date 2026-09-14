use crate::common::column_constraint_generator::ColumnConstraintGenerator;
use crate::common::column_generator::ColumnGenerator;
use crate::common::generator_context::GeneratorContext;
use crate::common::index_generator::IndexGenerator;
use crate::common::key_generator::KeyGenerator;
use crate::common::table_constraint_generator::TableConstraintGenerator;
use crate::{sql_newline, sql_print, sql_println};
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::table::Table;
use schema_model::model::types::DatabaseType;
use std::collections::{HashMap, HashSet, VecDeque};

pub trait TableGenerator {
    fn output_tables(&self);
    fn output_table(&self, table: &Table);
    fn output_table_drop(&self, table: &Table);
    fn output_table_header(&self, table: &Table);
    fn output_table_definition(&self, table: &Table);
    fn output_table_footer(&self, table: &Table);
    fn output_indexes(&self, table: &Table);
    fn output_initial_data(&self, table: &Table);
}

/// Returns every table across all schemas, ordered so that a table with a foreign key
/// to another table is listed before it (children before parents - the reverse of
/// dependency order).
///
/// Used to emit all `drop table` statements up front instead of interleaved with each
/// table's own `create` (H17): dropping in declaration order can try to drop a table
/// that another, not-yet-dropped table still holds a foreign key to, which fails on
/// SQL Server and SQLite (PostgreSQL is unaffected - its drop uses `cascade`).
///
/// A table caught in a dependency cycle (mutually-referencing foreign keys) falls back
/// to declaration order for the tables in that cycle - no ordering satisfies a cycle,
/// and this at least matches the pre-fix behavior instead of looping forever.
pub fn tables_in_drop_order(database_model: &DatabaseModel) -> Vec<&Table> {
    let tables = database_model.all_tables();
    let table_count = tables.len();

    let index_of_table: HashMap<*const Table, usize> = tables
        .iter()
        .enumerate()
        .map(|(index, table)| (*table as *const Table, index))
        .collect();

    let mut in_degree = vec![0usize; table_count];
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); table_count];

    for (child_index, table) in tables.iter().enumerate() {
        for relation in table.relations() {
            let Some(parent_table) = database_model.find_table_by_qualified_name_checked(relation.to_table_name()) else {
                continue;
            };
            let Some(&parent_index) = index_of_table.get(&(parent_table as *const Table)) else {
                continue;
            };

            if parent_index == child_index {
                continue; // self-referencing FK - a single drop handles both sides
            }

            successors[child_index].push(parent_index);
            in_degree[parent_index] += 1;
        }
    }

    let mut queue: VecDeque<usize> = (0..table_count).filter(|&index| in_degree[index] == 0).collect();
    let mut order = Vec::with_capacity(table_count);

    while let Some(index) = queue.pop_front() {
        order.push(index);

        for &successor in &successors[index] {
            in_degree[successor] -= 1;

            if in_degree[successor] == 0 {
                queue.push_back(successor);
            }
        }
    }

    if order.len() < table_count {
        let already_ordered: HashSet<usize> = order.iter().copied().collect();
        order.extend((0..table_count).filter(|index| !already_ordered.contains(index)));
    }

    order.into_iter().map(|index| tables[index]).collect()
}

pub struct DefaultTableGenerator {
    context: GeneratorContext,
    column_generator: Box<dyn ColumnGenerator>,
    key_generator: Box<dyn KeyGenerator>,
    column_constraint_generator: Box<dyn ColumnConstraintGenerator>,
    table_constraint_generator: Box<dyn TableConstraintGenerator>,
    index_generator: Box<dyn IndexGenerator>,
}

impl DefaultTableGenerator {
    pub fn new(
        context: GeneratorContext,
        column_generator: Box<dyn ColumnGenerator>,
        key_generator: Box<dyn KeyGenerator>,
        column_constraint_generator: Box<dyn ColumnConstraintGenerator>,
        table_constraint_generator: Box<dyn TableConstraintGenerator>,
        index_generator: Box<dyn IndexGenerator>,
    ) -> Self {
        Self {
            column_generator,
            key_generator,
            column_constraint_generator,
            table_constraint_generator,
            index_generator,
            context,
        }
    }

    /// Same as `output_table_definition`, but appends `extra` definition clauses
    /// (e.g. dialect-specific inline foreign key constraints) to the comma-separated
    /// list before printing.
    pub fn output_table_definition_with_extra(&self, table: &Table, extra: Vec<String>) {
        let table_definitions: Vec<String> = self.column_generator.column_definitions(table)
            .into_iter()
            .chain(self.key_generator.key_constraints(table))
            .chain(self.column_constraint_generator.column_check_constraints(table))
            .chain(self.table_constraint_generator.table_check_constraints(table))
            .chain(extra)
            .filter(|sql| !sql.is_empty())
            .collect();

        self.context.with_writer(|writer| {
            for (i, sql) in table_definitions.iter().enumerate() {
                sql_print!(writer, "{}", sql);

                if i < table_definitions.len() - 1 {
                    sql_print!(writer, ",");
                }

                sql_newline!(writer);
            }
        });
    }
}

impl TableGenerator for DefaultTableGenerator {
    fn output_tables(&self) {
        let database_model = self.context.settings().database_model();

        // All drops up front, in reverse-dependency order, rather than interleaved with
        // each table's own create (H17) - see `tables_in_drop_order`.
        for table in tables_in_drop_order(database_model) {
            self.output_table_drop(table);
        }

        for schema in database_model.schemas() {
            for table in schema.tables() {
                self.output_table_header(table);
                self.output_table_definition(table);
                self.output_table_footer(table);
                self.output_indexes(table);
                self.output_initial_data(table);
            };
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
        let is_postgres = self.context.settings().database_type() == DatabaseType::Postgresql;
        let cascade_suffix = if is_postgres { " cascade" } else { "" };
        let separator = self.context.settings().statement_separator().to_string();
        let fully_qualified_table_name = table.fully_qualified_table_name(self.context.settings().database_type());

        self.context.with_writer(|writer| {
            if is_postgres {
                sql_println!(writer, "/* {} */", fully_qualified_table_name);
                sql_println!(writer, "drop table if exists {}{}{}", fully_qualified_table_name, cascade_suffix, separator);
                sql_println!(writer, "");
            } else {
                sql_println!(writer, "drop table if exists {}{}{}", fully_qualified_table_name, cascade_suffix, separator);
                sql_println!(writer, "/* {} */", fully_qualified_table_name);
            }
        });
    }

    fn output_table_header(&self, table: &Table) {
        let fully_qualified_table_name = table.fully_qualified_table_name(self.context.settings().database_type());

        self.context.with_writer(|writer| {
            sql_println!(writer, "create table {}", fully_qualified_table_name);
            sql_println!(writer, "(");
        });
    }

    fn output_table_definition(&self, table: &Table) {
        self.output_table_definition_with_extra(table, Vec::new());
    }

    fn output_table_footer(&self, _table: &Table) {
        self.context.with_writer(|writer| {
            sql_println!(writer, "){}", self.context.settings().statement_separator());
            sql_newline!(writer);
        });
    }

    fn output_indexes(&self, table: &Table) {
        self.context.with_writer(|writer| {
            self.index_generator.output_indexes_for_table(writer, table);
        });
    }

    fn output_initial_data(&self, table: &Table) {
        let initial_data = table.initial_data()
            .iter()
            .filter(|it| { it.database_type().is_none() || it.database_type().unwrap() == self.context.settings().database_type() })
            .collect::<Vec<_>>();

        if !initial_data.is_empty() {
            self.context.with_writer(|writer| {
                initial_data.iter().for_each(|it| {
                    sql_println!(writer, "{}{}", it.sql(), self.context.settings().statement_separator());
                });

                writer.newline();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::column_generator::ColumnGenerator;
    use crate::common::index_generator::IndexGenerator;
    use crate::common::key_generator::KeyGenerator;
    use crate::common::table_constraint_generator::TableConstraintGenerator;
    use crate::common::test_support::make_context;
    use schema_model::builder::{SchemaBuilder, TableBuilder};
    use schema_model::model::column::Column;
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::initial_data::InitialData;
    use schema_model::model::key::Key;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    struct NoopColumnGenerator;
    impl ColumnGenerator for NoopColumnGenerator {
        fn column_definitions(&self, _table: &Table) -> Vec<String> { Vec::new() }
        fn column_sql(&self, _table: &Table, _column: &Column) -> String { String::new() }
        fn column_options(&self, _table: &Table, _column: &Column) -> String { String::new() }
        fn default_value(&self, _table: &Table, _column: &Column) -> Option<String> { None }
    }

    struct NoopKeyGenerator;
    impl KeyGenerator for NoopKeyGenerator {
        fn key_constraints(&self, _table: &Table) -> Vec<String> { Vec::new() }
    }

    struct NoopColumnConstraintGenerator;
    impl ColumnConstraintGenerator for NoopColumnConstraintGenerator {
        fn column_check_constraints(&self, _table: &Table) -> Vec<String> { Vec::new() }
    }

    /// Reproduces H6: a constraint generator that produces an empty string for a column
    /// that was flagged as needing a check constraint but ultimately has none (e.g. a
    /// Boolean column with `min`/`max` set under `BooleanMode::Native`).
    struct EmptyStringColumnConstraintGenerator;
    impl ColumnConstraintGenerator for EmptyStringColumnConstraintGenerator {
        fn column_check_constraints(&self, _table: &Table) -> Vec<String> { vec![String::new()] }
    }

    struct NoopTableConstraintGenerator;
    impl TableConstraintGenerator for NoopTableConstraintGenerator {
        fn table_check_constraints(&self, _table: &Table) -> Vec<String> { Vec::new() }
    }

    struct NoopIndexGenerator;
    impl IndexGenerator for NoopIndexGenerator {
        fn output_indexes(&self) {}
        fn output_indexes_for_table(&self, _writer: &mut crate::common::sql_writer::SqlWriter, _table: &Table) {}
        fn output_index(&self, _writer: &mut crate::common::sql_writer::SqlWriter, _statement_separator: &str, _table: &Table, _key_name: &str, _key: &Key) {}
        fn index_options(&self, _key: &Key) -> Option<String> { None }
    }

    fn make_generator(context: GeneratorContext) -> DefaultTableGenerator {
        DefaultTableGenerator::new(
            context,
            Box::new(NoopColumnGenerator),
            Box::new(NoopKeyGenerator),
            Box::new(NoopColumnConstraintGenerator),
            Box::new(NoopTableConstraintGenerator),
            Box::new(NoopIndexGenerator),
        )
    }

    #[test]
    fn output_initial_data_includes_rows_for_current_database_type_or_unspecified() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_initial_data(InitialData::new("insert into users values (1)", None))
            .add_initial_data(InitialData::new("insert into users values (2)", Some(DatabaseType::Sqlite)))
            .add_initial_data(InitialData::new("insert into users values (3)", Some(DatabaseType::Postgresql)))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = make_generator(ctx);
        generator.output_initial_data(&table);

        let output = buffer.contents();
        assert!(output.contains("insert into users values (1)"));
        assert!(output.contains("insert into users values (2)"));
        assert!(!output.contains("insert into users values (3)"));
    }

    #[test]
    fn output_initial_data_writes_nothing_when_table_has_no_rows() {
        let table = TableBuilder::new(None::<&str>, "users").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = make_generator(ctx);
        generator.output_initial_data(&table);

        assert_eq!(buffer.contents(), "");
    }

    #[test]
    fn output_table_runs_all_steps_in_order() {
        let table = TableBuilder::new(None::<&str>, "users")
            .add_initial_data(InitialData::new("insert into users values (1)", None))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = make_generator(ctx);
        generator.output_table(&table);

        let output = buffer.contents();
        let header_pos = output.find("create table users").unwrap();
        let footer_pos = output.find(")").unwrap();
        let data_pos = output.find("insert into users values (1)").unwrap();
        assert!(header_pos < footer_pos);
        assert!(footer_pos < data_pos);
    }

    #[test]
    fn output_table_definition_drops_empty_constraint_strings() {
        let table = TableBuilder::new(None::<&str>, "orders").build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, buffer) = make_context(model, DatabaseType::Sqlite);

        let generator = DefaultTableGenerator::new(
            ctx,
            Box::new(NoopColumnGenerator),
            Box::new(NoopKeyGenerator),
            Box::new(EmptyStringColumnConstraintGenerator),
            Box::new(NoopTableConstraintGenerator),
            Box::new(NoopIndexGenerator),
        );
        generator.output_table_definition(&table);

        let output = buffer.contents();
        // No stray leading comma and no blank line between "(" and ")" from the empty entry.
        assert!(!output.contains(","));
        assert!(!output.contains("\n\n"));
    }

    fn table_names(tables: Vec<&Table>) -> Vec<String> {
        tables.iter().map(|t| t.name().to_string()).collect()
    }

    #[test]
    fn tables_in_drop_order_puts_child_before_parent() {
        use schema_model::model::relation::Relation;
        use schema_model::model::types::RelationType;

        // Declared in create order (parent before the child that references it) -
        // drop order must be the reverse.
        let parent = TableBuilder::new(None::<&str>, "parent").build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(parent).add_table(child).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        assert_eq!(table_names(tables_in_drop_order(&model)), vec!["child", "parent"]);
    }

    #[test]
    fn tables_in_drop_order_handles_chain_of_three() {
        use schema_model::model::relation::Relation;
        use schema_model::model::types::RelationType;

        let grandparent = TableBuilder::new(None::<&str>, "grandparent").build();
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_relation(Relation::new("grandparent", "id", "parent", "grandparent_id", RelationType::Cascade, false))
            .build();
        let child = TableBuilder::new(None::<&str>, "child")
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(grandparent)
            .add_table(parent)
            .add_table(child)
            .build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        assert_eq!(table_names(tables_in_drop_order(&model)), vec!["child", "parent", "grandparent"]);
    }

    #[test]
    fn tables_in_drop_order_ignores_self_referencing_relation() {
        use schema_model::model::relation::Relation;
        use schema_model::model::types::RelationType;

        // e.g. employee.manager_id -> employee.id - must not be treated as a cycle
        // against itself (which would leave it stuck with a permanent in-degree).
        let employee = TableBuilder::new(None::<&str>, "employee")
            .add_relation(Relation::new("employee", "id", "employee", "manager_id", RelationType::SetNull, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(employee).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        assert_eq!(table_names(tables_in_drop_order(&model)), vec!["employee"]);
    }

    #[test]
    fn tables_in_drop_order_falls_back_to_declaration_order_on_a_cycle() {
        use schema_model::model::relation::Relation;
        use schema_model::model::types::RelationType;

        // "a" references "b" and "b" references "a" - no ordering satisfies both, so
        // this must not panic or loop forever, and should fall back predictably.
        let a = TableBuilder::new(None::<&str>, "a")
            .add_relation(Relation::new("b", "id", "a", "b_id", RelationType::SetNull, false))
            .build();
        let b = TableBuilder::new(None::<&str>, "b")
            .add_relation(Relation::new("a", "id", "b", "a_id", RelationType::SetNull, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(a).add_table(b).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        assert_eq!(table_names(tables_in_drop_order(&model)), vec!["a", "b"]);
    }
}
