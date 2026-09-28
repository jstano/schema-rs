use crate::common::output_mode::OutputMode;
use crate::common::print_writer::PrintWriter;
use schema_model::model::database_model::DatabaseModel;
use schema_model::model::types::{BooleanMode, ForeignKeyMode};
use std::cell::RefCell;
use std::rc::Rc;

pub struct GenerateOptions {
    pub database_model: Rc<DatabaseModel>,
    pub writer: Rc<RefCell<PrintWriter>>,
    pub foreign_key_mode: ForeignKeyMode,
    pub boolean_mode: BooleanMode,
    pub output_mode: OutputMode,
    pub target_postgres_version: u32,
    pub emit_postgres_extensions: bool,
    pub extension_check_user: Option<String>,
    /// SQL Server's product year (e.g. 2019, 2022, 2025), used to gate features not
    /// available on every supported version - currently just the native `json` type,
    /// introduced in SQL Server 2025 (see `json_sql`). `0` means "unset", which is treated
    /// as "target the widest range of servers" (pre-2025 behavior).
    pub target_sqlserver_version: u32,
}

impl GenerateOptions {
    pub fn new(database_model: Rc<DatabaseModel>, writer: Rc<RefCell<PrintWriter>>) -> Self {
        Self {
            database_model,
            writer,
            foreign_key_mode: ForeignKeyMode::Relations,
            boolean_mode: BooleanMode::Native,
            output_mode: OutputMode::All,
            target_postgres_version: 0,
            emit_postgres_extensions: true,
            extension_check_user: None,
            target_sqlserver_version: 0,
        }
    }
}
