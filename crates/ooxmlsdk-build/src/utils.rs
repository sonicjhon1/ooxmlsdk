use crate::error::{BuildError, BuildErrorReport};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::parse2;

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

pub fn use_traits_fn() -> Result<TokenStream, BuildErrorReport> {
    return Ok(parse2(quote! {
        use crate::common::Deserializeable;
    })
    .map_err(BuildError::from)?);
}
