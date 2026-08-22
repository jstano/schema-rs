use crate::model::enum_type::EnumType;
use crate::model::schema::Schema;
use crate::model::table::Table;
use crate::model::types::{BooleanMode, ForeignKeyMode};

#[derive(Debug, Default)]
pub struct DatabaseModel {
    foreign_key_mode: ForeignKeyMode,
    boolean_mode: BooleanMode,
    schemas: Vec<Schema>,
}

impl DatabaseModel {
    pub fn new(boolean_mode: BooleanMode,
               foreign_key_mode: ForeignKeyMode,
               schemas: Vec<Schema>) -> Self {
        Self {
            boolean_mode,
            foreign_key_mode,
            schemas,
        }
    }

    pub fn foreign_key_mode(&self) -> ForeignKeyMode {
        self.foreign_key_mode
    }

    pub fn boolean_mode(&self) -> BooleanMode {
        self.boolean_mode
    }

    pub fn schemas(&self) -> &Vec<Schema> {
        &self.schemas
    }

    pub fn default_schema(&self) -> &Schema {
        self.schemas
            .iter()
            .find(|s| s.schema_name().is_none())
            .expect("Default schema not found")
    }

    pub fn default_schema_mut(&mut self) -> &mut Schema {
        self.schemas
            .iter_mut()
            .find(|s| s.schema_name().is_none())
            .expect("Default schema not found")
    }

    pub fn find_schema(&self, schema_name: Option<&str>) -> &Schema {
        if schema_name.is_none() {
            return self.default_schema();
        }

        self.schemas
            .iter()
            .filter(|s| s.schema_name().is_some())
            .find(|s| s.schema_name().unwrap() == schema_name.unwrap())
            .expect("Schema not found")
    }

    pub(crate) fn find_schema_mut(&mut self, schema_name: Option<&str>) -> &mut Schema {
        if schema_name.is_none() {
            return self.default_schema_mut();
        }

        self.schemas
            .iter_mut()
            .filter(|s| s.schema_name().is_some())
            .find(|s| s.schema_name().unwrap() == schema_name.unwrap())
            .expect("Schema not found")
    }

    pub fn find_enum_type(&self, schema_name: Option<&str>, enum_type: &str) -> &EnumType {
        let schema = self.find_schema(schema_name);
        schema.get_enum_type(enum_type)
    }

    pub fn all_tables(&self) -> Vec<&Table> {
        self.schemas
            .iter()
            .flat_map(|s| s.tables())
            .collect()
    }

    pub fn find_table(&self, schema_name: Option<&str>, table_name: &str) -> &Table {
        let schema = self.find_schema(schema_name);
        schema.get_table(table_name)
    }

    pub fn find_table_by_qualified_name(&self, qualified_name: &str) -> &Table {
        let parts: Vec<&str> = qualified_name.split('.').collect();
        let (schema_name, table_name) = if parts.len() == 2 {
            (Some(parts[0]), parts[1])
        } else {
            (None, qualified_name)
        };

        self.find_table(schema_name, table_name)
    }

    /// Same as `find_table_by_qualified_name`, but returns `None` instead of panicking
    /// when the schema or table doesn't exist - used by `validate()` to check relation
    /// targets without crashing on a dangling one.
    pub fn find_table_by_qualified_name_checked(&self, qualified_name: &str) -> Option<&Table> {
        let parts: Vec<&str> = qualified_name.split('.').collect();
        let (schema_name, table_name) = if parts.len() == 2 {
            (Some(parts[0]), parts[1])
        } else {
            (None, qualified_name)
        };

        self.schemas
            .iter()
            .find(|s| s.schema_name() == schema_name)
            .and_then(|s| s.get_optional_table(table_name))
    }

    /// Checks model-wide invariants that individual `Schema::validate()` calls can't see
    /// (a relation's target table may live in a different schema than the one declaring
    /// it). Combined with each schema's own `validate()`, this is meant to be run once
    /// after parsing/building a model and before handing it to SQL generation or
    /// migration - several generator code paths (`find_enum_type`, `find_table_by_qualified_name`,
    /// ...) panic on a reference that doesn't resolve, on the assumption the model was
    /// already validated.
    pub fn validate(&self) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();

        for schema in &self.schemas {
            errors.extend(schema.validate());
        }

        for table in self.all_tables() {
            for relation in table.relations() {
                if self.find_table_by_qualified_name_checked(relation.to_table_name()).is_none() {
                    errors.push(format!(
                        "ERROR: {}.{} has a relation to '{}' which does not exist",
                        table.name(),
                        relation.from_column_name(),
                        relation.to_table_name()
                    ));
                }
            }
        }

        errors
    }

    pub fn find_table_mut(&mut self, schema_name: Option<&str>, table_name: &str) -> &mut Table {
        let schema = self.find_schema_mut(schema_name);
        schema.get_table_mut(table_name)
    }

    /// Same as `find_table_mut`, but returns `None` instead of panicking when the schema
    /// or table doesn't exist - useful for callers (e.g. the XML parser) resolving
    /// possibly-malformed input, where a dangling reference should surface as an error
    /// rather than crash.
    pub fn find_table_mut_checked(&mut self, schema_name: Option<&str>, table_name: &str) -> Option<&mut Table> {
        let schema = self.schemas.iter_mut().find(|s| s.schema_name() == schema_name)?;
        schema.get_table_mut_checked(table_name)
    }

    pub fn sort_tables_by_name(&mut self) {
        for schema in self.schemas.iter_mut() {
            schema.sort_tables_by_name();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{ColumnBuilder, SchemaBuilder, TableBuilder};
    use crate::model::column_type::ColumnType;
    use crate::model::relation::Relation;
    use crate::model::types::RelationType;

    #[test]
    fn validate_reports_error_for_a_relation_whose_target_table_does_not_exist() {
        // A relation whose target table doesn't exist anywhere in the model (typo'd
        // name, or a schema that was never declared) must surface as a validation
        // error - several generator code paths panic on this instead if it's not
        // caught up front.
        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("Regoin", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(child).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);

        let errors = model.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Regoin"));
    }

    #[test]
    fn validate_accepts_a_relation_whose_target_table_exists_in_a_different_schema() {
        let parent = TableBuilder::new(Some("app"), "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .build();
        let parent_schema = SchemaBuilder::new(Some("app")).add_table(parent).build();

        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).build())
            .add_relation(Relation::new("app.parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();
        let default_schema = SchemaBuilder::new(None::<&str>).add_table(child).build();

        let model = DatabaseModel::new(
            BooleanMode::Native,
            ForeignKeyMode::Relations,
            vec![default_schema, parent_schema],
        );

        assert!(model.validate().is_empty());
    }
}
