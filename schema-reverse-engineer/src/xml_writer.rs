use schema_model::model::column::Column;
use schema_model::model::constraint::Constraint;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::enum_type::EnumType;
use schema_model::model::key::Key;
use schema_model::model::relation::Relation;
use schema_model::model::schema::Schema;
use schema_model::model::table::Table;
use schema_model::model::types::{BooleanMode, ForeignKeyMode, KeyType};
use schema_model::model::view::View;
use std::fmt::Write as _;

const NAMESPACE: &str = "http://stano.com/database";

/// Serializes a `DatabaseModel` to the schema-rs XML schema definition format (see
/// `schema-xsd/schema.xsd`), the inverse of `schema_parser::parse_database_xml`.
pub fn write_database_xml(model: &DatabaseModel) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<database xmlns=\"{}\" foreignKeyMode=\"{}\" booleanMode=\"{}\">",
        NAMESPACE,
        foreign_key_mode_str(model.foreign_key_mode()),
        boolean_mode_str(model.boolean_mode())
    );

    for schema in model.schemas() {
        write_schema_body(&mut out, schema, 1);
    }

    out.push_str("</database>\n");
    out
}

fn write_schema_body(out: &mut String, schema: &Schema, indent: usize) {
    for enum_type in schema.enum_types() {
        write_enum(out, enum_type, indent);
    }
    for table in schema.tables() {
        write_table(out, table, indent);
    }
    for view in schema.all_views() {
        write_view(out, view, indent);
    }
}

fn write_enum(out: &mut String, enum_type: &EnumType, indent: usize) {
    push_indent(out, indent);
    let _ = writeln!(out, "<enum name=\"{}\">", xml_escape(enum_type.name()));
    for value in enum_type.values() {
        push_indent(out, indent + 1);
        let _ = write!(out, "<value name=\"{}\"", xml_escape(value.name()));
        if value.code() != value.name() {
            let _ = write!(out, " code=\"{}\"", xml_escape(value.code()));
        }
        out.push_str("/>\n");
    }
    push_indent(out, indent);
    out.push_str("</enum>\n\n");
}

fn write_view(out: &mut String, view: &View, indent: usize) {
    push_indent(out, indent);
    let _ = writeln!(out, "<view name=\"{}\">", xml_escape(view.name()));
    push_indent(out, indent + 1);
    write_cdata(out, view.sql());
    out.push('\n');
    push_indent(out, indent);
    out.push_str("</view>\n\n");
}

fn write_table(out: &mut String, table: &Table, indent: usize) {
    push_indent(out, indent);
    let _ = writeln!(out, "<table name=\"{}\">", xml_escape(table.name()));

    write_columns(out, table.columns(), indent + 1);
    write_keys(out, table, indent + 1);
    write_relations(out, table.relations(), indent + 1);
    write_constraints(out, table.constraints(), indent + 1);

    push_indent(out, indent);
    out.push_str("</table>\n\n");
}

fn write_columns(out: &mut String, columns: &[Column], indent: usize) {
    push_indent(out, indent);
    out.push_str("<columns>\n");
    for column in columns {
        write_column(out, column, indent + 1);
    }
    push_indent(out, indent);
    out.push_str("</columns>\n");
}

fn write_column(out: &mut String, column: &Column, indent: usize) {
    push_indent(out, indent);
    let _ = write!(
        out,
        "<column name=\"{}\" type=\"{}\"",
        xml_escape(column.name()),
        column.column_type().name().to_lowercase()
    );
    if column.length() > 0 {
        let _ = write!(out, " length=\"{}\"", column.length());
    }
    if column.scale() > 0 {
        let _ = write!(out, " scale=\"{}\"", column.scale());
    }
    if column.required() {
        out.push_str(" required=\"true\"");
    }
    if let Some(default) = column.default_constraint() {
        let _ = write!(out, " default=\"{}\"", xml_escape(default));
    }
    if let Some(generated) = column.generated() {
        let _ = write!(out, " generated=\"{}\"", xml_escape(generated));
    }
    if let Some(enum_type) = column.enum_type() {
        let _ = write!(out, " enumType=\"{}\"", xml_escape(enum_type));
    }
    if let Some(element_type) = column.element_type() {
        let _ = write!(out, " elementType=\"{}\"", xml_escape(element_type));
    }
    if let Some(min_value) = column.min_value() {
        let _ = write!(out, " minValue=\"{}\"", min_value);
    }
    if let Some(max_value) = column.max_value() {
        let _ = write!(out, " maxValue=\"{}\"", max_value);
    }

    if let Some(check) = column.check_constraint() {
        out.push_str(">\n");
        push_indent(out, indent + 1);
        let _ = writeln!(out, "<check>{}</check>", xml_escape(check));
        push_indent(out, indent);
        out.push_str("</column>\n");
    } else {
        out.push_str("/>\n");
    }
}

fn write_keys(out: &mut String, table: &Table, indent: usize) {
    // The XSD requires a <primary> element whenever <keys> is present, so a table with no
    // primary key (and no promoted single-unique-key) can't emit unique/index keys either.
    let Some(primary) = table.primary_key() else {
        return;
    };

    push_indent(out, indent);
    out.push_str("<keys>\n");
    write_key_columns(out, "primary", primary, indent + 1);
    for unique in table.keys().iter().filter(|k| k.key_type() == KeyType::Unique) {
        write_key_columns(out, "unique", unique, indent + 1);
    }
    for index in table.indexes() {
        write_index(out, index, indent + 1);
    }
    push_indent(out, indent);
    out.push_str("</keys>\n");
}

fn write_key_columns(out: &mut String, tag: &str, key: &Key, indent: usize) {
    push_indent(out, indent);
    let _ = writeln!(out, "<{}>", tag);
    for column in key.columns() {
        push_indent(out, indent + 1);
        let _ = writeln!(out, "<column name=\"{}\"/>", xml_escape(column.name()));
    }
    push_indent(out, indent);
    let _ = writeln!(out, "</{}>", tag);
}

fn write_index(out: &mut String, index: &Key, indent: usize) {
    push_indent(out, indent);
    let _ = write!(out, "<index");
    if index.is_unique() {
        out.push_str(" unique=\"true\"");
    }
    if let Some(include) = index.include() {
        let _ = write!(out, " include=\"{}\"", xml_escape(include));
    }
    out.push_str(">\n");
    for column in index.columns() {
        push_indent(out, indent + 1);
        let _ = writeln!(out, "<column name=\"{}\"/>", xml_escape(column.name()));
    }
    push_indent(out, indent);
    out.push_str("</index>\n");
}

fn write_relations(out: &mut String, relations: &[Relation], indent: usize) {
    if relations.is_empty() {
        return;
    }
    push_indent(out, indent);
    out.push_str("<relations>\n");
    for relation in relations {
        push_indent(out, indent + 1);
        let _ = writeln!(
            out,
            "<relation src=\"{}\" table=\"{}\" column=\"{}\" type=\"{}\"/>",
            xml_escape(relation.from_column_name()),
            xml_escape(relation.to_table_name()),
            xml_escape(relation.to_column_name()),
            relation_type_str(relation.relation_type())
        );
    }
    push_indent(out, indent);
    out.push_str("</relations>\n");
}

fn write_constraints(out: &mut String, constraints: &[Constraint], indent: usize) {
    if constraints.is_empty() {
        return;
    }
    push_indent(out, indent);
    out.push_str("<constraints>\n");
    for constraint in constraints {
        push_indent(out, indent + 1);
        let _ = writeln!(
            out,
            "<constraint name=\"{}\" databaseType=\"{}\">",
            xml_escape(constraint.name()),
            database_type_str(constraint.database_type())
        );
        push_indent(out, indent + 2);
        write_cdata(out, constraint.sql());
        out.push('\n');
        push_indent(out, indent + 1);
        out.push_str("</constraint>\n");
    }
    push_indent(out, indent);
    out.push_str("</constraints>\n");
}

fn relation_type_str(relation_type: schema_model::model::types::RelationType) -> &'static str {
    use schema_model::model::types::RelationType;
    match relation_type {
        RelationType::Cascade => "cascade",
        RelationType::Enforce => "enforce",
        RelationType::SetNull => "setnull",
        RelationType::DoNothing => "donothing",
    }
}

fn boolean_mode_str(mode: BooleanMode) -> &'static str {
    match mode {
        BooleanMode::Native => "native",
        BooleanMode::YesNo => "yesno",
        BooleanMode::YN => "yn",
    }
}

fn database_type_str(database_type: schema_model::model::types::DatabaseType) -> &'static str {
    use schema_model::model::types::DatabaseType;
    match database_type {
        DatabaseType::Postgresql => "postgresql",
        DatabaseType::Sqlite => "sqlite",
        DatabaseType::SqlServer => "sqlserver",
    }
}

fn foreign_key_mode_str(mode: ForeignKeyMode) -> &'static str {
    match mode {
        ForeignKeyMode::None => "none",
        ForeignKeyMode::Relations => "relations",
        ForeignKeyMode::Triggers => "triggers",
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

/// Writes `content` as a `<![CDATA[...]]>` section, splitting any embedded `]]>` across
/// adjacent CDATA sections first. The literal sequence `]]>` is illegal inside XML
/// character data (not just "outside CDATA") - e.g. SQL text containing an array-slice
/// like `arr[1:2]]>x` would otherwise close the CDATA section early and produce
/// non-well-formed XML that fails to parse back.
fn write_cdata(out: &mut String, content: &str) {
    out.push_str("<![CDATA[");
    out.push_str(&content.replace("]]>", "]]]]><![CDATA[>"));
    out.push_str("]]>");
}

/// XML-escapes attribute and text content consistently (the Java writer inconsistently escaped
/// index/unique key column names but not primary key column names -- here every column name and
/// text value goes through the same escaping).
fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema_model::builder::{ColumnBuilder, KeyBuilder, SchemaBuilder, TableBuilder};
    use schema_model::model::column_type::ColumnType;
    use schema_model::model::enum_type::EnumValue;
    use schema_model::model::relation::Relation;
    use schema_model::model::types::{DatabaseType, RelationType};

    #[test]
    fn writes_table_with_columns_pk_and_relation() {
        let parent = TableBuilder::new(None::<&str>, "parent")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .build();

        let child = TableBuilder::new(None::<&str>, "child")
            .add_column(ColumnBuilder::new(None::<&str>, "id", ColumnType::Sequence).required(true).build())
            .add_column(ColumnBuilder::new(None::<&str>, "parent_id", ColumnType::Int).required(true).build())
            .add_key(KeyBuilder::new(KeyType::Primary).add_column("id").build())
            .add_relation(Relation::new("parent", "id", "child", "parent_id", RelationType::Cascade, false))
            .build();

        let schema = SchemaBuilder::new(None::<&str>)
            .add_table(parent)
            .add_table(child)
            .add_enum_type(EnumType::new("mood", vec![EnumValue::new("HAPPY", None::<String>)]))
            .add_view(View::new(None::<&str>, "v1", "select 1", Some(DatabaseType::Postgresql)))
            .build();

        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let xml = write_database_xml(&model);

        assert!(xml.contains("<database xmlns=\"http://stano.com/database\""));
        assert!(xml.contains("<table name=\"parent\">"));
        assert!(xml.contains("<column name=\"id\" type=\"sequence\" required=\"true\"/>"));
        assert!(xml.contains("<relation src=\"parent_id\" table=\"parent\" column=\"id\" type=\"cascade\"/>"));
        assert!(xml.contains("<enum name=\"mood\">"));
        assert!(xml.contains("<view name=\"v1\">"));
    }

    #[test]
    fn write_cdata_splits_embedded_close_sequence() {
        let mut out = String::new();
        write_cdata(&mut out, "select arr[1:2]]>x from t");
        assert!(!out.contains("2]]>x"), "the literal ]]> must not survive unescaped inside one CDATA block: {out}");
        assert!(out.starts_with("<![CDATA["));
        assert!(out.ends_with("]]>"));
    }

    #[test]
    fn view_sql_containing_cdata_close_sequence_round_trips_through_the_real_parser() {
        let view = View::new(
            None::<&str>,
            "v1",
            "select * from t where arr[1:2]]>x",
            None,
        );
        let schema = SchemaBuilder::new(None::<&str>).add_view(view).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let xml = write_database_xml(&model);

        let reparsed = schema_parser::parse_database_xml(&xml)
            .expect("XML containing a ]]> sequence inside a view's SQL must still be well-formed");
        let reparsed_view = &reparsed.default_schema().all_views()[0];
        assert_eq!(reparsed_view.sql(), "select * from t where arr[1:2]]>x");
    }

    #[test]
    fn constraint_sql_containing_cdata_close_sequence_round_trips_through_the_real_parser() {
        let table = TableBuilder::new(None::<&str>, "t")
            .add_column(ColumnBuilder::new(None::<&str>, "a", ColumnType::Int).build())
            .add_constraint(schema_model::model::constraint::Constraint::new(
                "ck_a",
                "check (a[1:2]]>0)",
                DatabaseType::Postgresql,
            ))
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let xml = write_database_xml(&model);

        let reparsed = schema_parser::parse_database_xml(&xml)
            .expect("XML containing a ]]> sequence inside a constraint's SQL must still be well-formed");
        let reparsed_table = reparsed.default_schema().get_table("t");
        assert_eq!(reparsed_table.constraints()[0].sql(), "check (a[1:2]]>0)");
    }

    #[test]
    fn omits_keys_when_no_primary_key_present() {
        let table = TableBuilder::new(None::<&str>, "t")
            .add_column(ColumnBuilder::new(None::<&str>, "a", ColumnType::Int).build())
            .build();
        let schema = SchemaBuilder::new(None::<&str>).add_table(table).build();
        let model = DatabaseModel::new(BooleanMode::Native, ForeignKeyMode::Relations, vec![schema]);
        let xml = write_database_xml(&model);
        assert!(!xml.contains("<keys>"));
    }
}
