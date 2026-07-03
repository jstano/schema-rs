use crate::change::SchemaChange;

#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    changes: Vec<SchemaChange>,
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_change(&mut self, change: SchemaChange) {
        self.changes.push(change);
    }

    pub fn changes(&self) -> &[SchemaChange] {
        &self.changes
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }
}
