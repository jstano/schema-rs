#[derive(Debug, Clone)]
pub struct EnumType {
    name: String,
}
impl EnumType {
    pub fn new<S: Into<String>>(name: S) -> Self { Self { name: name.into() } }
    pub fn name(&self) -> &str { &self.name }
}

#[derive(Debug, Clone)]
pub struct EnumValue {
    name: String,
    code: Option<String>,
}

impl EnumValue {
    /// Create a new EnumValue. `code` may be None to mirror Java's nullable field.
    pub fn new<N: Into<String>, C: Into<String>>(name: N, code: Option<C>) -> Self {
        Self { name: name.into(), code: code.map(|c| c.into()) }
    }

    pub fn name(&self) -> &str { &self.name }

    /// Returns the explicit code if present; otherwise falls back to the name,
    /// matching the Java getter behavior.
    pub fn code(&self) -> &str { self.code.as_deref().unwrap_or(&self.name) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_type_and_value_getters() {
        let et = EnumType::new("Color");
        assert_eq!(et.name(), "Color");

        let v1 = EnumValue::new("RED", None::<String>);
        assert_eq!(v1.name(), "RED");
        // falls back to name
        assert_eq!(v1.code(), "RED");

        let v2 = EnumValue::new("GREEN", Some("G"));
        assert_eq!(v2.code(), "G");
    }
}
