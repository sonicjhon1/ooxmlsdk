use crate::error::BuildError;
use heck::{ToSnakeCase, ToUpperCamelCase};
use std::collections::HashMap;

pub fn escape_snake_case(name: &str) -> String {
    let name = name.to_snake_case();

    match name.as_str() {
        "if" | "else" | "ref" | "type" | "macro" | "loop" | "mod" | "override" | "for" | "in"
        | "box" | "final" | "break" => {
            format!("r#{name}")
        }
        _ => name,
    }
}

pub fn escape_upper_camel_case(name: &str) -> String {
    let name = name.to_upper_camel_case();

    match name.as_str() {
        "self" | "Self" => {
            format!("_{name}")
        }
        _ => name,
    }
}

pub trait HashMapOpsError<K, V> {
    fn try_get(&self, key: K) -> Result<&V, BuildError>;
    fn try_get_mut(&mut self, key: K) -> Result<&mut V, BuildError>;
}

impl<K: AsRef<str>, V> HashMapOpsError<K, V> for HashMap<&str, V> {
    fn try_get(&self, key: K) -> Result<&V, BuildError> {
        return self
            .get(key.as_ref())
            .ok_or_else(|| BuildError::HashMapExpectedSomeError(key.as_ref().to_string()));
    }

    fn try_get_mut(&mut self, key: K) -> Result<&mut V, BuildError> {
        return self
            .get_mut(key.as_ref())
            .ok_or_else(|| BuildError::HashMapExpectedSomeError(key.as_ref().to_string()));
    }
}

impl<K: AsRef<str>, V> HashMapOpsError<K, V> for HashMap<String, V> {
    fn try_get(&self, key: K) -> Result<&V, BuildError> {
        return self
            .get(key.as_ref())
            .ok_or_else(|| BuildError::HashMapExpectedSomeError(key.as_ref().to_string()));
    }

    fn try_get_mut(&mut self, key: K) -> Result<&mut V, BuildError> {
        return self
            .get_mut(key.as_ref())
            .ok_or_else(|| BuildError::HashMapExpectedSomeError(key.as_ref().to_string()));
    }
}
