use std::rc::Rc;
use schema_model::model::database_model::DatabaseModel;

pub struct DiagramGenerateOptions {
    pub database_model: Rc<DatabaseModel>,
}
