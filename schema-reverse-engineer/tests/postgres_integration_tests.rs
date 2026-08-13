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
"#;

async fn setup_pool() -> (testcontainers::ContainerAsync<Postgres>, PgPool) {
    let postgres = Postgres::default().start().await.expect("postgres container should start");
    let port = postgres.get_host_port_ipv4(5432).await.expect("get mapped port");
    let connection_string = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&connection_string)
        .await
        .expect("connect to postgres");

    sqlx::raw_sql(SCHEMA_SQL).execute(&pool).await.expect("schema setup should succeed");

    (postgres, pool)
}

#[tokio::test]
async fn test_reverse_engineer_postgres_schema() {
    if !docker_tests_enabled() {
        eprintln!("skipping test_reverse_engineer_postgres_schema: set RUN_DOCKER_TESTS=1 to run");
        return;
    }

    let (_container, pool) = setup_pool().await;

    let model = read_schema(&pool, "public").await.expect("introspection should succeed");
    let schema = model.default_schema();

    let customers = schema.get_table("customers");
    assert_eq!(customers.column("id").column_type(), ColumnType::Uuid);
    assert_eq!(customers.column("tags").column_type(), ColumnType::Array);
    assert_eq!(customers.column("tags").element_type(), Some("text"));
    assert_eq!(customers.column("profile").column_type(), ColumnType::Json);
    assert_eq!(customers.column("current_mood").column_type(), ColumnType::Enum);
    assert_eq!(customers.column("current_mood").enum_type(), Some("mood"));
    assert!(customers.primary_key().is_some());
    assert!(customers
        .keys()
        .iter()
        .any(|k| k.key_type() == KeyType::Unique && k.contains_column("email")));

    let orders = schema.get_table("orders");
    assert_eq!(orders.column("id").column_type(), ColumnType::Sequence);
    assert_eq!(orders.column("amount").column_type(), ColumnType::Decimal);
    assert_eq!(orders.column("amount").length(), 10);
    assert_eq!(orders.column("amount").scale(), 2);
    assert!(!orders.constraints().is_empty(), "check constraint on amount should be captured");
    assert!(orders.indexes().iter().any(|k| k.contains_column("line_no")));

    let relation = orders.relations().first().expect("fk relation present");
    assert_eq!(relation.from_table_name(), "orders");
    assert_eq!(relation.from_column_name(), "customer_id");
    assert_eq!(relation.to_table_name(), "customers");
    assert_eq!(relation.to_column_name(), "id");
    assert_eq!(relation.relation_type(), RelationType::Cascade);

    assert!(schema.enum_types().any(|e| e.name() == "mood"));
    assert!(schema.all_views().iter().any(|v| v.name() == "order_totals"));

    // Round-trip the generated XML back through the existing parser to make sure the writer
    // produces XML the rest of the toolkit can actually read.
    let xml = write_database_xml(&model);
    let parsed = schema_parser::parse_database_xml(&xml).expect("generated xml should parse");
    let parsed_schema = parsed.default_schema();
    assert!(parsed_schema.get_optional_table("customers").is_some());
    assert!(parsed_schema.get_optional_table("orders").is_some());
    let parsed_orders = parsed_schema.get_table("orders");
    assert_eq!(parsed_orders.relations().len(), 1);
    assert_eq!(parsed_orders.relations()[0].relation_type(), RelationType::Cascade);
}
