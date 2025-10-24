use crate::model::key::{Key, KeyColumn};
use crate::model::types::KeyType;

/// KeyBuilder collects column names and attributes, producing a model::Key.
#[derive(Debug)]
pub struct KeyBuilder {
    key_type: KeyType,
    columns: Vec<KeyColumn>,
    cluster: bool,
    compress: bool,
    unique: bool,
    include: Option<String>,
}

impl KeyBuilder {
    pub fn new(key_type: KeyType) -> Self {
        Self {
            key_type,
            columns: Vec::new(),
            cluster: false,
            compress: false,
            unique: false,
            include: None,
        }
    }
    pub fn add_column<S: Into<String>>(mut self, name: S) -> Self {
        self.columns.push(KeyColumn::new(name));
        self
    }
    pub fn cluster(mut self, v: bool) -> Self {
        self.cluster = v;
        self
    }
    pub fn compress(mut self, v: bool) -> Self {
        self.compress = v;
        self
    }
    pub fn unique(mut self, v: bool) -> Self {
        self.unique = v;
        self
    }
    pub fn include<S: Into<String>>(mut self, s: S) -> Self {
        self.include = Some(s.into());
        self
    }

    pub fn build(self) -> Key {
        if self.cluster || self.compress || self.unique || self.include.is_some() {
            Key::new_full(
                self.key_type,
                self.columns,
                self.cluster,
                self.compress,
                self.unique,
                self.include,
            )
        } else {
            Key::new(self.key_type, self.columns)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_index_with_include() {
        let k = KeyBuilder::new(KeyType::Index)
            .add_column("a")
            .add_column("b")
            .compress(true)
            .include("x")
            .build();
        assert_eq!(k.columns().len(), 2);
        assert!(k.is_compress());
        assert_eq!(k.include(), Some("x"));
    }
}
