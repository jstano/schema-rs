use crate::model::types::{DatabaseType, TriggerType};

#[derive(Debug, Clone)]
pub struct Trigger {
    trigger_text: String,
    trigger_type: TriggerType,
    database_type: DatabaseType,
}

impl Trigger {
    pub fn new<S: Into<String>>(
        trigger_text: S,
        trigger_type: TriggerType,
        database_type: DatabaseType,
    ) -> Self {
        Self {
            trigger_text: trigger_text.into(),
            trigger_type,
            database_type,
        }
    }

    pub fn trigger_text(&self) -> &str {
        &self.trigger_text
    }
    pub fn trigger_type(&self) -> TriggerType {
        self.trigger_type
    }
    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_getters() {
        let t = Trigger::new("trg", TriggerType::Delete, DatabaseType::Mysql);
        assert_eq!(t.trigger_text(), "trg");
        assert_eq!(t.trigger_type(), TriggerType::Delete);
        assert_eq!(t.database_type(), DatabaseType::Mysql);
    }
}
