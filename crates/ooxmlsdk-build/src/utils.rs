use heck::{ToSnakeCase, ToUpperCamelCase};

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

macro_rules! get_or_panic {
    ($map:expr_2021, $key:expr_2021) => {
        $map.get($key).ok_or_else(|| format!("{:?}", $key)).unwrap()
    };
}

pub(crate) use get_or_panic;
