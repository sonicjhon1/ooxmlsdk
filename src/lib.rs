#![feature(const_trait_impl)]
#![feature(const_option_ops)]
#![feature(if_let_guard)]

pub mod common {
    include!(concat!(env!("OUT_DIR"), "/common/mod.rs"));
}

pub mod schemas {
    include!(concat!(env!("OUT_DIR"), "/schemas/mod.rs"));
}

pub mod deserializers {
    include!(concat!(env!("OUT_DIR"), "/deserializers/mod.rs"));
}

pub mod serializers {
    include!(concat!(env!("OUT_DIR"), "/serializers/mod.rs"));
}

#[cfg(feature = "parts")]
pub mod parts {
    include!(concat!(env!("OUT_DIR"), "/parts/mod.rs"));
}

pub mod taggable {
    include!(concat!(env!("OUT_DIR"), "/tagger/mod.rs"));
}

#[cfg(feature = "validators")]
pub mod validators {
    include!(concat!(env!("OUT_DIR"), "/validators/mod.rs"));
}
