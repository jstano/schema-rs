use crate::model::types::KeyType;

#[derive(Debug, Clone)]
pub struct KeyColumn {
    name: String,
}

impl KeyColumn {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct Key {
    r#type: KeyType,
    columns: Vec<KeyColumn>,
    cluster: bool,
    compress: bool,
    unique: bool,
    include: Option<String>,
}

impl Key {
    pub fn new(r#type: KeyType, columns: Vec<KeyColumn>) -> Self {
        Self {
            r#type,
            columns,
            cluster: false,
            compress: false,
            unique: false,
            include: None,
        }
    }

    pub fn new_full<S: Into<String>>(
        r#type: KeyType,
        columns: Vec<KeyColumn>,
        cluster: bool,
        compress: bool,
        unique: bool,
        include: Option<S>,
    ) -> Self {
        Self {
            r#type,
            columns,
            cluster,
            compress,
            unique,
            include: include.map(|s| s.into()),
        }
    }

    pub fn r#type(&self) -> KeyType {
        self.r#type
    }
    pub fn columns(&self) -> &Vec<KeyColumn> {
        &self.columns
    }
    pub fn is_cluster(&self) -> bool {
        self.cluster
    }
    pub fn is_compress(&self) -> bool {
        self.compress
    }
    pub fn is_unique(&self) -> bool {
        self.unique
    }
    pub fn include(&self) -> Option<&str> {
        self.include.as_deref()
    }

    pub fn contains_column(&self, column_name: &str) -> bool {
        self.columns.iter().any(|c| c.name() == column_name)
    }

    pub fn columns_as_string(&self) -> String {
        self.columns
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::KeyType;

    #[test]
    fn key_column_and_key_basics() {
        let cols = vec![KeyColumn::new("id"), KeyColumn::new("code")];
        let k = Key::new(KeyType::Primary, cols.clone());
        assert_eq!(k.r#type(), KeyType::Primary);
        assert_eq!(k.columns().len(), 2);
        assert!(!k.is_cluster());
        assert!(!k.is_compress());
        assert!(!k.is_unique());
        assert_eq!(k.include(), None);
        assert!(k.contains_column("id"));
        assert_eq!(k.columns_as_string(), "id,code");

        let k2 = Key::new_full(KeyType::Index, cols, true, true, true, Some("inc"));
        assert!(k2.is_cluster());
        assert!(k2.is_compress());
        assert!(k2.is_unique());
        assert_eq!(k2.include(), Some("inc"));
    }
}
