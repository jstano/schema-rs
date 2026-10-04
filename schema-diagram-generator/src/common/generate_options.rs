use schema_model::model::database_model::DatabaseModel;
use std::rc::Rc;

pub struct DiagramGenerateOptions {
    pub database_model: Rc<DatabaseModel>,
}
