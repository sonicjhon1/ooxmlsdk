use proc_macro2::Literal;
use quote::quote;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use syn::{ItemConst, parse_quote};

use crate::{error::*, models::*, utils::gen_use_common_glob};

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

    // TODO: Is this needed?
    // let xml_needs_header =
    //     !schema_type.part.is_empty() || schema_type.base_class == "OpenXmlPartRootElement";

    let type_items: &[ItemConst] = match type_prefix {
        "op" => &[],
        prefix => {
            let prefixed_name = Literal::string(&format!("{type_name}:{type_prefix}"));
            let prefix = Literal::string(prefix);

            &[
                parse_quote!(const PREFIXED_NAME: Option<&str> = Some(#prefixed_name);),
                parse_quote!(const PREFIX: Option<&str> = Some(#prefix);),
            ]
        }
    };

    return Ok(quote!(
      impl Taggable for #struct_type {
          #( #type_items )*
          const NAME: &str = #type_name;
      }
    )
    .to_string());
}
