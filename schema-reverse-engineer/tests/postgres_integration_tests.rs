use schema_model::model::column_type::ColumnType;
use schema_model::model::types::{KeyType, RelationType};
use schema_reverse_engineer::{read_schema, write_database_xml};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

fn docker_tests_enabled() -> bool {
    std::env::var("RUN_DOCKER_TESTS").is_ok()
}

const SCHEMA_SQL: &str = r#"
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy');

CREATE TABLE customers (
    id uuid PRIMARY KEY,
    name varchar(100) NOT NULL,
    tags text[],
    profile jsonb,
    current_mood mood,
    email varchar(255) NOT NULL,
    CONSTRAINT customers_email_key UNIQUE (email)
);

CREATE TABLE orders (
    id serial,
    customer_id uuid NOT NULL,
    line_no int4 NOT NULL,
    amount numeric(10,2) NOT NULL CHECK (amount >= 0),
    PRIMARY KEY (id),
    CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
);

CREATE INDEX idx_orders_line_no ON orders (line_no);

CREATE VIEW order_totals AS
SELECT customer_id, sum(amount) AS total FROM orders GROUP BY customer_id;

CREATE FUNCTION double_it(x int) RETURNS int LANGUAGE sql IMMUTABLE AS 'SELECT x * 2';

CREATE PROCEDURE noop_proc() LANGUAGE sql AS 'SELECT 1';
"#;

async fn setup_pool() -> (testcontainers::ContainerAsync<Postgres>, PgPool) {
    let postgres = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = postgres
        .get_host_port_ipv4(5432)
        .await
        .expect("get mapped port");
    let connection_string = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&connection_string)
        .await
        .expect("connect to postgres");

    sqlx::raw_sql(SCHEMA_SQL)
        .execute(&pool)
        .await
        .expect("schema setup should succeed");

    (postgres, pool)
}

#[tokio::test]
async fn test_reverse_engineer_postgres_schema() {
    if !docker_tests_enabled() {
        eprintln!("skipping test_reverse_engineer_postgres_schema: set RUN_DOCKER_TESTS=1 to run");
        return;
    }

    let (_container, pool) = setup_pool().await;

    let model = read_schema(&pool, "public")
        .await
        .expect("introspection should succeed");
    let schema = model.find_schema(Some("public"));
    assert_eq!(schema.schema_name(), Some("public"));

    let customers = schema.get_table("customers");
    assert_eq!(customers.column("id").column_type(), ColumnType::Uuid);
    assert_eq!(customers.column("tags").column_type(), ColumnType::Array);
    assert_eq!(customers.column("tags").element_type(), Some("text"));
    assert_eq!(customers.column("profile").column_type(), ColumnType::Json);
    assert_eq!(
        customers.column("current_mood").column_type(),
        ColumnType::Enum
    );
    assert_eq!(customers.column("current_mood").enum_type(), Some("mood"));
    assert!(customers.primary_key().is_some());
    assert!(
        customers
            .keys()
            .iter()
            .any(|k| k.key_type() == KeyType::Unique && k.contains_column("email"))
    );

    let orders = schema.get_table("orders");
    assert_eq!(orders.column("id").column_type(), ColumnType::Sequence);
    assert_eq!(orders.column("amount").column_type(), ColumnType::Decimal);
    assert_eq!(orders.column("amount").length(), 10);
    assert_eq!(orders.column("amount").scale(), 2);
    assert!(
        !orders.constraints().is_empty(),
        "check constraint on amount should be captured"
    );
    assert!(
        orders
            .indexes()
            .iter()
            .any(|k| k.contains_column("line_no"))
    );

    let relation = orders.relations().first().expect("fk relation present");
    assert_eq!(relation.from_table_name(), "orders");
    assert_eq!(relation.from_column_name(), "customer_id");
    assert_eq!(relation.to_table_name(), "customers");
    assert_eq!(relation.to_column_name(), "id");
    assert_eq!(relation.relation_type(), RelationType::Cascade);

    assert!(schema.enum_types().any(|e| e.name() == "mood"));
    assert!(
        schema
            .functions()
            .iter()
            .any(|f| f.name() == "double_it" && f.sql().contains("x * 2"))
    );
    assert!(schema.procedures().iter().any(|p| p.name() == "noop_proc"));
    assert!(
        schema
            .all_views()
            .iter()
            .any(|v| v.name() == "order_totals")
    );

    // Round-trip the generated XML back through the existing parser to make sure the writer
    // produces XML the rest of the toolkit can actually read.
    let xml = write_database_xml(&model);
    let parsed = schema_parser::parse_database_xml(&xml).expect("generated xml should parse");
    let parsed_schema = parsed.find_schema(Some("public"));
    assert!(parsed_schema.get_optional_table("customers").is_some());
    assert!(parsed_schema.get_optional_table("orders").is_some());
    let parsed_orders = parsed_schema.get_table("orders");
    assert_eq!(parsed_orders.relations().len(), 1);
    assert_eq!(
        parsed_orders.relations()[0].relation_type(),
        RelationType::Cascade
    );
}

const M33_SQL: &str = r#"
CREATE SCHEMA other;
CREATE TABLE other.foreign_parent (id int PRIMARY KEY);

CREATE TABLE events (
    id int PRIMARY KEY,
    code text,
    name text,
    deleted_at timestamp,
    other_id int,
    CONSTRAINT fk_events_other FOREIGN KEY (other_id) REFERENCES other.foreign_parent (id)
);
CREATE UNIQUE INDEX ux_events_code_live ON events (code) WHERE deleted_at IS NULL;
CREATE INDEX ix_events_code_covering ON events (code) INCLUDE (name);
CREATE INDEX ix_events_lower_name ON events (lower(name));

-- lone unique key on a nullable column must not become a primary key
CREATE TABLE nullable_unique (email text, CONSTRAINT nu_email UNIQUE (email));
-- lone unique key on a NOT NULL column is still promoted
CREATE TABLE required_unique (email text NOT NULL, CONSTRAINT ru_email UNIQUE (email));

CREATE TABLE measurements (
    id int NOT NULL,
    logged date NOT NULL,
    PRIMARY KEY (id, logged)
) PARTITION BY RANGE (logged);
CREATE INDEX ix_measurements_logged ON measurements (logged);

CREATE VIEW plain_view AS SELECT id FROM events;
CREATE MATERIALIZED VIEW mat_view AS SELECT id FROM events;
"#;

#[tokio::test]
async fn test_reverse_engineer_m33_edge_cases() {
    if !docker_tests_enabled() {
        eprintln!("skipping test_reverse_engineer_m33_edge_cases: set RUN_DOCKER_TESTS=1 to run");
        return;
    }

    let (_container, pool) = setup_pool().await;
    sqlx::raw_sql(M33_SQL)
        .execute(&pool)
        .await
        .expect("m33 schema setup should succeed");

    let model = read_schema(&pool, "public")
        .await
        .expect("introspection should succeed");
    let schema = model.find_schema(Some("public"));

    // Cross-schema FK keeps a schema-qualified target.
    let events = schema.get_table("events");
    assert_eq!(events.relations().len(), 1);
    assert_eq!(
        events.relations()[0].to_table_name(),
        "other.foreign_parent"
    );

    // Partial unique index keeps its predicate; covering index keeps INCLUDE columns;
    // expression index is skipped.
    let partial = events
        .indexes()
        .iter()
        .find(|k| k.filter().is_some())
        .expect("partial index present");
    assert!(partial.is_unique());
    assert_eq!(partial.columns().len(), 1);
    assert!(partial.filter().unwrap().contains("deleted_at IS NULL"));
    let covering = events
        .indexes()
        .iter()
        .find(|k| k.include().is_some())
        .expect("covering index present");
    assert_eq!(covering.columns().len(), 1);
    assert_eq!(covering.include(), Some("name"));
    assert_eq!(
        events.indexes().len(),
        2,
        "expression index must be skipped"
    );

    // Promotion only when the column is already NOT NULL.
    assert!(schema.get_table("nullable_unique").primary_key().is_none());
    // ...and its unique key is skipped (with a warning) when writing, not a hard error.
    assert!(write_database_xml(&model).contains("nullable_unique"));
    assert!(schema.get_table("required_unique").primary_key().is_some());

    // Partitioned tables keep their keys and indexes.
    let measurements = schema.get_table("measurements");
    assert!(measurements.primary_key().is_some());
    assert!(
        measurements
            .indexes()
            .iter()
            .any(|k| k.contains_column("logged"))
    );

    // Materialized view skipped, plain view kept.
    assert!(schema.all_views().iter().any(|v| v.name() == "plain_view"));
    assert!(!schema.all_views().iter().any(|v| v.name() == "mat_view"));
}

#[tokio::test]
async fn test_reverse_engineer_cross_schema_fk_xml() {
    if !docker_tests_enabled() {
        eprintln!(
            "skipping test_reverse_engineer_cross_schema_fk_xml: set RUN_DOCKER_TESTS=1 to run"
        );
        return;
    }

    let (_container, pool) = setup_pool().await;
    sqlx::raw_sql(
        "CREATE SCHEMA other; \
         CREATE TABLE other.foreign_parent (id int PRIMARY KEY); \
         CREATE TABLE child (id int PRIMARY KEY, other_id int REFERENCES other.foreign_parent (id));",
    )
    .execute(&pool)
    .await
    .expect("setup");

    // Each schema is reverse-engineered separately; the cross-schema target stays qualified.
    let public_xml = write_database_xml(&read_schema(&pool, "public").await.unwrap());
    assert!(
        public_xml.contains("table=\"other.foreign_parent\""),
        "got: {public_xml}"
    );
    let other_xml = write_database_xml(&read_schema(&pool, "other").await.unwrap());
    assert!(other_xml.contains("foreign_parent"));
}
