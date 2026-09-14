use crate::common::column_type_generator::ColumnTypeGenerator;
use crate::common::constraint_naming;
use crate::common::generator_context::GeneratorContext;
use schema_model::model::column::Column;
use schema_model::model::column_type::ColumnType;
use schema_model::model::table::Table;
use schema_model::model::types::BooleanMode;

const DF_PREFIX: &str = "df_";

/// Controls how a column's `default` constraint is named in generated DDL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultConstraintNaming {
    /// `default {value}` - no named constraint (e.g. Postgres).
    Unnamed,
    /// `constraint {column} default {value}` - named after the bare column (e.g. SQLite).
    NamedByColumn,
    /// `constraint df_{table[0..9]}_{column[0..9]}_{HASH8} default {value}` - matches the
    /// legacy Java codegen tool's SQL Server naming scheme.
    NamedWithHash,
}

pub trait ColumnGenerator {
    fn column_definitions(&self, table: &Table) -> Vec<String>;

    fn column_sql(&self, table: &Table, column: &Column) -> String;

    fn column_options(&self, table: &Table, column: &Column) -> String;

    fn default_value(&self, table: &Table, column: &Column) -> Option<String>;
}

pub struct DefaultColumnGenerator {
    context: GeneratorContext,
    column_type_generator: Box<dyn ColumnTypeGenerator>,
    default_constraint_naming: DefaultConstraintNaming,
}

impl DefaultColumnGenerator {
    pub fn new(
        context: GeneratorContext,
        column_type_generator: Box<dyn ColumnTypeGenerator>,
        default_constraint_naming: DefaultConstraintNaming,
    ) -> Self {
        Self {
            column_type_generator,
            context,
            default_constraint_naming,
        }
    }

    fn convert_boolean_default_constraint(&self, value: bool) -> String {
        match self.context.settings().boolean_mode() {
            BooleanMode::Native => self.column_type_generator.native_boolean_literal(value),
            BooleanMode::YesNo => {
                if value {
                    "'Yes'".to_string()
                } else {
                    "'No'".to_string()
                }
            }
            BooleanMode::YN => {
                if value {
                    "'Y'".to_string()
                } else {
                    "'N'".to_string()
                }
            }
        }
    }

    fn boolean_default_value(&self, default_constraint: Option<&str>) -> Option<String> {
        // No `default` attribute at all means no default constraint - unlike the old
        // behaviour here, this must *not* fall back to `default false`: that made a nullable
        // boolean column with no default inexpressible (H2). `parse_boolean_default` already
        // recognizes the `null` sentinel as "no default constraint" (`Ok(None)`), distinct
        // from this "attribute absent" case.
        let default_constraint = default_constraint?;

        // `Schema::validate()` rejects any value `parse_boolean_default` can't parse before
        // generation ever runs, so `Err(())` here is unreachable through the CLI/installer -
        // but a caller going straight to the generator without validating first should still
        // get a working (if surprising) `false` rather than a panic.
        match schema_model::model::column::parse_boolean_default(default_constraint) {
            Ok(Some(value)) => Some(self.convert_boolean_default_constraint(value)),
            Ok(None) => None,
            Err(()) => Some(self.convert_boolean_default_constraint(false)),
        }
    }

    fn uuid_default_value(
        &self,
        table: &Table,
        column: &Column,
        default_constraint: Option<&str>,
    ) -> Option<String> {
        let schema = self
            .context
            .settings()
            .database_model()
            .find_schema(table.schema_name());
        let primary_key_columns = table.primary_key_columns();

        if column.required()
            && primary_key_columns.is_some_and(|columns| columns.contains(&column.name().to_string()))
            && table.column_relation(column).is_none()
        {
            return Some(self.column_type_generator.uuid_default_value_sql(schema));
        }

        if default_constraint.is_some_and(|dc| dc.eq_ignore_ascii_case("generate_uuid()")) {
            return Some(self.column_type_generator.uuid_default_value_sql(schema));
        }

        None
    }

    fn default_constraint(&self, table: &Table, column: &Column, default_value: &str) -> String {
        match self.default_constraint_naming {
            DefaultConstraintNaming::Unnamed => format!("default {}", default_value),
            DefaultConstraintNaming::NamedByColumn => {
                format!("constraint {} default {}", column.name(), default_value)
            }
            DefaultConstraintNaming::NamedWithHash => {
                let constraint_name =
                    constraint_naming::hashed_constraint_name(DF_PREFIX, table.name(), column.name());
                format!("constraint {} default {}", constraint_name, default_value)
            }
        }
    }
}

impl ColumnGenerator for DefaultColumnGenerator {
    fn column_definitions(&self, table: &Table) -> Vec<String> {
        table
            .columns()
            .iter()
            .map(|column| self.column_sql(table, column))
            .collect()
    }

    fn column_sql(&self, table: &Table, column: &Column) -> String {
        let column_options = self.column_options(table, column);

        if column_options.is_empty() {
            return format!(
                "   {} {}",
                column.name(),
                self.column_type_generator.column_type_sql(table, column)
            );
        }

        format!(
            "   {} {} {}",
            column.name(),
            self.column_type_generator.column_type_sql(table, column),
            column_options
        )
    }

    fn column_options(&self, table: &Table, column: &Column) -> String {
        let mut options = String::new();

        if column.required() {
            if column.length() > 0 {
                options.push(' ');
            }

            options.push_str("not null")
        }

        let default_value = self.default_value(table, column);

        if let Some(default_value) = default_value {
            if !options.is_empty() {
                options.push(' ');
            }

            options.push_str(self.default_constraint(table, column, default_value.as_ref()).as_str());
        }

        options.trim().to_string()
    }

    fn default_value(&self, table: &Table, column: &Column) -> Option<String> {
        let default_constraint = column.default_constraint();

        match column.column_type() {
            ColumnType::Boolean => self.boolean_default_value(default_constraint),
            ColumnType::Uuid => self.uuid_default_value(table, column, default_constraint),
            _ => default_constraint.map(String::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::make_context;
    use schema_model::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::database_model::DatabaseModel;
    use schema_model::model::schema::Schema;
    use schema_model::model::types::{BooleanMode, DatabaseType, ForeignKeyMode};

    /// Bare-minimum `ColumnTypeGenerator` - only `native_boolean_sql` matters here, the rest
    /// exist purely to satisfy the trait's other required methods.
    struct DummyColumnTypeGenerator(GeneratorContext);
    impl ColumnTypeGenerator for DummyColumnTypeGenerator {
        fn context(&self) -> &GeneratorContext { &self.0 }
        fn sequence_sql(&self) -> String { String::new() }
        fn long_sequence_sql(&self) -> String { String::new() }
        fn text_sql(&self, _column: &Column) -> String { String::new() }
        fn citext_sql(&self) -> String { String::new() }
        fn cstext_sql(&self) -> String { String::new() }
        fn binary_sql(&self) -> String { String::new() }
        fn uuid_default_value_sql(&self, _schema: &Schema) -> String { String::new() }
        fn array_sql(&self, _column: &Column) -> String { String::new() }
        fn json_sql(&self, _column: &Column) -> String { String::new() }
        fn native_boolean_sql(&self) -> String { "boolean".to_string() }
    }

    fn boolean_column_default(default_constraint: Option<&str>) -> Option<String> {
        let mut builder = ColumnBuilder::new(None::<&str>, "active", ColumnType::Boolean);
        if let Some(default_constraint) = default_constraint {
            builder = builder.default_constraint(Some(default_constraint.to_string()));
        }
        let table = TableBuilder::new(None::<&str>, "widget").add_column(builder.build()).build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table.clone()).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let (ctx, _buffer) = make_context(model, DatabaseType::Postgresql);

        let generator = DefaultColumnGenerator::new(
            ctx.clone(),
            Box::new(DummyColumnTypeGenerator(ctx)),
            DefaultConstraintNaming::Unnamed,
        );
        let column = table.columns()[0].clone();
        generator.default_value(&table, &column)
    }

    #[test]
    fn boolean_column_with_no_default_attribute_gets_no_default_constraint() {
        // Regression test for H2: omitting `default` used to silently force `default false`,
        // making a nullable boolean column with no default inexpressible.
        assert_eq!(boolean_column_default(None), None);
    }

    #[test]
    fn boolean_column_default_null_sentinel_means_no_default_constraint() {
        assert_eq!(boolean_column_default(Some("null")), None);
        assert_eq!(boolean_column_default(Some("NULL")), None);
    }

    #[test]
    fn boolean_column_default_recognizes_truthy_spellings() {
        for value in ["true", "TRUE", " true ", "1", "yes", "on"] {
            assert_eq!(boolean_column_default(Some(value)), Some("true".to_string()), "value: {:?}", value);
        }
    }

    #[test]
    fn boolean_column_default_recognizes_falsy_spellings() {
        for value in ["false", "FALSE", "0", "no", "off"] {
            assert_eq!(boolean_column_default(Some(value)), Some("false".to_string()), "value: {:?}", value);
        }
    }

    #[test]
    fn boolean_column_default_falls_back_to_false_for_an_unrecognized_value() {
        // `Schema::validate()` rejects this before generation ever runs (see schema-model's
        // `Schema::validate` tests), so this only exercises the defensive fallback for a
        // caller that invokes the generator directly without validating first.
        assert_eq!(boolean_column_default(Some("maybe")), Some("false".to_string()));
    }
}
