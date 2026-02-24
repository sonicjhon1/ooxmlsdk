use crate::{error::*, models::*, utils::gen_use_common_glob};
use proc_macro2::Literal;
use quote::quote;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use syn::{ItemConst, parse_quote};

pub fn gen_tagger(schema: &OpenXmlSchema) -> Result<String, BuildErrorReport> {
    let mut contents = String::with_capacity(const { 128 * 64 });

    if !schema.types.is_empty() {
        contents.push_str(&gen_use_common_glob().to_string());
    }

    contents.push_str(
        &schema
            .types
            .par_iter()
            .map(|schema_type| gen_schema_type(schema, schema_type))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
    );

    Ok(contents)
}

fn gen_schema_type(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
) -> Result<String, BuildErrorReport> {
    if schema_type.is_abstract {
        return Ok(String::with_capacity(0));
    }

    let struct_type = schema.struct_type(schema_type);

    let (type_prefix, type_name) = schema_type.split_last_name();

    let prefixed_items: &[ItemConst] = match type_prefix {
        "op" => &[],
        prefix => {
            let namespace_uri = Literal::string(schema.target_namespace.as_str());
            let prefixed_name = Literal::string(&format!("{type_prefix}:{type_name}"));
            let prefix = Literal::string(prefix);

            &[
                parse_quote! {
                    const NAMESPACE_URI: Option<&str> = Some(#namespace_uri);
                },
                parse_quote! {
                    const PREFIXED_NAME: Option<&str> = Some(#prefixed_name);
                },
                parse_quote! {
                    const PREFIX: Option<&str> = Some(#prefix);
                },
            ]
        }
    };

    let needs_header_item: Option<ItemConst> = (!schema_type.part.is_empty()
        || schema_type.base_class == "OpenXmlPartRootElement")
        .then_some(parse_quote! {
            const NEEDS_HEADER: bool = true;
        });

    return Ok(quote! {
        impl Taggable for #struct_type {
            #( #prefixed_items )*
            const NAME: &str = #type_name;
            #needs_header_item
        }
    }
    .to_string());
}
