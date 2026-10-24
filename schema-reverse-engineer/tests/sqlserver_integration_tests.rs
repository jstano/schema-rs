use schema_model::model::column_type::ColumnType;
use schema_model::model::types::{DatabaseType, KeyType, OtherSqlOrder, RelationType};
use schema_reverse_engineer::sqlserver::{self, SqlServerClient};
use schema_reverse_engineer::{read_sqlserver_schema, write_database_xml};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mssql_server::MssqlServer;

// SQL Server's Linux container crashes under QEMU emulation (Apple Silicon), so this is ignored
// by default. On a native x86_64 host/CI runner:
//   cargo test -p schema-reverse-engineer --test sqlserver_integration_tests -- --ignored
// To run against a server you already have (e.g. an ARM-native azure-sql-edge container), set
// SQLSERVER_TEST_CONNECTION_STRING to its ADO connection string.

const SCHEMA_SQL: &[&str] = &[
    // Idempotent, so the test can be re-run against a long-lived server.
    "DROP PROCEDURE IF EXISTS dbo.noop_proc",
    "DROP FUNCTION IF EXISTS dbo.double_it",
    "DROP VIEW IF EXISTS dbo.order_totals",
    "DROP TABLE IF EXISTS dbo.order_notes",
    "DROP TABLE IF EXISTS dbo.orders",
    "DROP TABLE IF EXISTS dbo.customers",
    "CREATE TABLE customers (
        id uniqueidentifier NOT NULL PRIMARY KEY,
        name nvarchar(100) NOT NULL,
        notes nvarchar(max) NULL,
        email nvarchar(255) NOT NULL,
        balance decimal(10,2) NOT NULL DEFAULT 0,
        active bit NOT NULL DEFAULT 1,
        CONSTRAINT uq_customers_email UNIQUE (email)
    )",
    "CREATE TABLE orders (
        id int IDENTITY(1,1) NOT NULL,
        customer_id uniqueidentifier NOT NULL,
        line_no int NOT NULL,
        quantity int NOT NULL,
        unit_price decimal(10,2) NOT NULL,
        total AS (quantity * unit_price) PERSISTED,
        status varchar(10) NOT NULL DEFAULT 'new',
        amount decimal(10,2) NOT NULL CONSTRAINT ck_orders_amount CHECK (amount >= 0),
        created datetime2 NOT NULL,
        PRIMARY KEY (id),
        CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
    )",
    "CREATE INDEX idx_orders_line_no ON orders (line_no)",
    "CREATE UNIQUE INDEX idx_orders_status ON orders (status) INCLUDE (line_no) WHERE status <> 'new'",
    "CREATE TABLE order_notes (
        id int NOT NULL,
        order_id int NULL,
        note_no int NOT NULL,
        note nvarchar(200) NULL,
        PRIMARY KEY (id, note_no),
        CONSTRAINT fk_notes_order FOREIGN KEY (order_id) REFERENCES orders (id) ON DELETE SET NULL
    )",
    "CREATE VIEW order_totals AS SELECT customer_id, SUM(amount) AS total FROM dbo.orders GROUP BY customer_id",
    "CREATE FUNCTION dbo.double_it(@x int) RETURNS int AS BEGIN RETURN @x * 2 END",
    "CREATE PROCEDURE dbo.noop_proc AS BEGIN SELECT 1 END",
    // Dropped along with its table above.
    "CREATE TRIGGER dbo.trg_order_notes_audit ON dbo.order_notes AFTER INSERT, UPDATE AS BEGIN SELECT 1 END",
];

async fn connect() -> (
    Option<testcontainers::ContainerAsync<MssqlServer>>,
    SqlServerClient,
) {
    let (container, connection_string) = match std::env::var("SQLSERVER_TEST_CONNECTION_STRING") {
        Ok(cs) => (None, cs),
        Err(_) => {
            let mssql = MssqlServer::default()
                .start()
                .await
                .expect("mssql container should start");
            let port = mssql.get_host_port_ipv4(1433).await.expect("mapped port");
            let cs = format!(
                "Server=localhost,{port};Database=master;User Id=sa;Password=yourStrong(!)Password;TrustServerCertificate=true;"
            );
            (Some(mssql), cs)
        }
    };
    let mut client = sqlserver::connect(&connection_string, true)
        .await
        .expect("connect to sql server");
    for statement in SCHEMA_SQL {
        client
            .simple_query(*statement)
            .await
            .unwrap_or_else(|e| panic!("setup failed for {statement}: {e}"))
            .into_results()
            .await
            .expect("setup results");
    }
    (container, client)
}

#[tokio::test]
#[ignore]
async fn test_reverse_engineer_sqlserver_schema() {
    let (_container, mut client) = connect().await;

    let model = read_sqlserver_schema(&mut client, "dbo")
        .await
        .expect("introspection should succeed");
    let schema = model.find_schema(Some("dbo"));

    let customers = schema.get_table("customers");
    assert_eq!(customers.column("id").column_type(), ColumnType::Uuid);
    assert_eq!(customers.column("name").column_type(), ColumnType::Varchar);
    assert_eq!(customers.column("name").length(), 100);
    assert_eq!(customers.column("notes").column_type(), ColumnType::Text);
    assert!(!customers.column("notes").required());
    assert_eq!(
        customers.column("balance").column_type(),
        ColumnType::Decimal
    );
    assert_eq!(customers.column("balance").length(), 10);
    assert_eq!(customers.column("balance").scale(), 2);
    assert_eq!(customers.column("balance").default_constraint(), Some("0"));
    assert_eq!(
        customers.column("active").column_type(),
        ColumnType::Boolean
    );
    assert!(customers.primary_key().is_some());
    assert!(
        customers
            .keys()
            .iter()
            .any(|k| k.key_type() == KeyType::Unique && k.contains_column("email"))
    );

    let orders = schema.get_table("orders");
    assert_eq!(orders.column("id").column_type(), ColumnType::Sequence);
    assert_eq!(
        orders.column("created").column_type(),
        ColumnType::Timestamp
    );
    assert_eq!(orders.column("status").default_constraint(), Some("'new'"));
    assert_eq!(
        orders.column("total").generated(),
        Some("[quantity]*[unit_price]")
    );
    assert!(orders.column("total").default_constraint().is_none());
    assert!(
        orders
            .constraints()
            .iter()
            .any(|c| c.name() == "ck_orders_amount"
                && c.sql().starts_with("check (")
                && c.database_type() == Some(DatabaseType::SqlServer))
    );
    assert!(
        orders
            .indexes()
            .iter()
            .any(|k| k.contains_column("line_no") && !k.is_unique())
    );
    let filtered = orders
        .indexes()
        .iter()
        .find(|k| k.contains_column("status"))
        .expect("filtered unique index is an index, not a unique key");
    assert!(filtered.is_unique());
    assert!(filtered.include().is_some());
    assert!(filtered.filter().is_some());

    let relation = orders.relations().first().expect("fk relation present");
    assert_eq!(relation.to_table_name(), "customers");
    assert_eq!(relation.relation_type(), RelationType::Cascade);

    let notes = schema.get_table("order_notes");
    assert_eq!(notes.primary_key().unwrap().columns().len(), 2);
    assert!(!notes.column("order_id").required());
    assert_eq!(notes.relations()[0].relation_type(), RelationType::SetNull);

    assert!(
        schema.all_views().iter().any(|v| {
            v.name() == "order_totals" && v.sql().to_lowercase().starts_with("select")
        })
    );
    assert!(schema.functions().iter().any(|f| f.name() == "double_it"));
    assert!(schema.procedures().iter().any(|p| p.name() == "noop_proc"));
    assert!(
        schema.other_sql().iter().any(
            |o| o.order() == OtherSqlOrder::Bottom && o.sql().contains("trg_order_notes_audit")
        )
    );

    // The written XML must be accepted by the real parser.
    let xml = write_database_xml(&model);
    let reparsed = schema_parser::parse_database_xml(&xml)
        .unwrap_or_else(|e| panic!("generated XML should reparse: {e}\n{xml}"));
    assert!(
        reparsed
            .find_schema(Some("dbo"))
            .get_table("orders")
            .column("total")
            .generated()
            .is_some()
    );
}
