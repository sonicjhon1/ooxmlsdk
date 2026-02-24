use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rootcause::report;
use std::collections::HashMap;
use syn::{Ident, ImplItemFn, Stmt, Type, parse_quote};

use crate::{
    GenContext,
    error::*,
    models::*,
    utils::{HashMapOpsError, gen_use_common_glob},
};

pub fn gen_serializer(
    schema: &OpenXmlSchema,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    let mut contents = String::with_capacity(const { 256 * 1024 });

    if !schema.types.is_empty() {
        contents.push_str("#![allow(clippy::possible_missing_else)]\n");
        contents.push_str(&gen_use_common_glob().to_string());
    }

    contents.push_str(
        &schema
            .types
            .par_iter()
            .map(|schema_type| gen_schema_type(schema, schema_type, gen_context))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
    );

    contents.push_str(
        &schema
            .enums
            .par_iter()
            .map(|schema_enum| gen_schema_enum(schema, schema_enum))
            .collect::<Result<Vec<_>, BuildErrorReport>>()?
            .join("\n"),
    );

    Ok(contents)
}

fn gen_schema_type(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    if schema_type.is_abstract {
        return Ok(String::with_capacity(0));
    }

    let struct_type = schema.struct_type(schema_type);

    let with_xmlns_ident = format_ident!("with_xmlns");

    let xml_attr_ident = format_ident!("attr");
    let mut xml_attr_impl_item_fn: Option<ImplItemFn> = None;
    let xml_inner_ident = format_ident!("xml");
    let mut xml_inner_impl_item_fn: Option<ImplItemFn> = None;

    let mut xml_attr_writers: Vec<TokenStream> = Vec::with_capacity(schema_type.attributes.len());

    if schema.needs_xmlns(schema_type) {
        xml_attr_writers.push(quote! {
            #xml_attr_ident.push_str(&self.xmlns.serialize_attributes(#with_xmlns_ident));
        })
    }

    for resolved_schema_type_attribute in
        schema_type.resolved_schema_type_attributes(gen_context)?
    {
        let ResolvedSchemaTypeAttribute {
            field_name_literal_string,
            field_name_ident,
            ..
        } = resolved_schema_type_attribute;

        if resolved_schema_type_attribute.is_validator_required {
            xml_attr_writers.push(quote! {
                #xml_attr_ident.push_str(
                    &as_xml_attribute(#field_name_literal_string, &quick_xml::escape::escape(self.#field_name_ident.to_string()))
                );
            })
        } else {
            xml_attr_writers.push(quote! {
              if let Some(#field_name_ident) = &self.#field_name_ident {
                #xml_attr_ident.push_str(
                    &as_xml_attribute(#field_name_literal_string, &quick_xml::escape::escape(#field_name_ident.to_string()))
                );
              }
            })
        };
    }

    let xml_inner_writer = gen_inner_writer(
        schema,
        schema_type,
        &xml_attr_ident,
        &mut xml_attr_writers,
        &xml_inner_ident,
        gen_context,
    )?;

    if !xml_attr_writers.is_empty() {
        xml_attr_impl_item_fn = Some(parse_quote! {
            #[allow(unused_variables)]
            fn xml_tag_attributes(&self, #with_xmlns_ident: bool) -> Option<String> {
                let mut #xml_attr_ident = String::new();

                #( #xml_attr_writers )*

                return Some(#xml_attr_ident);
            }
        });
    }

    if xml_inner_writer.is_some() {
        xml_inner_impl_item_fn = Some(parse_quote!(
            #[allow(unused_variables)]
            fn xml_inner(&self, #with_xmlns_ident: bool) -> Option<String> {
                let mut #xml_inner_ident = String::with_capacity(512);

                #xml_inner_writer

                return Some(#xml_inner_ident);
            }
        ))
    };

    return Ok(quote!(
        impl Serializeable for #struct_type {
            #xml_attr_impl_item_fn

            #xml_inner_impl_item_fn
        }
    )
    .to_string());
}

fn gen_schema_enum(
    schema: &OpenXmlSchema,
    schema_enum: &OpenXmlSchemaEnum,
) -> Result<String, BuildErrorReport> {
    let enum_type = schema.enum_type(schema_enum);

    let variants = schema_enum.facets.iter().map(|schema_enum_facet| {
        let variant_ident = schema_enum_facet.as_variant_ident();
        let variant_value = &schema_enum_facet.value;

        return quote! {
          Self::#variant_ident => #variant_value,
        };
    });

    return Ok(quote! {
      impl std::fmt::Display for #enum_type {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          let value = match self {
            #( #variants )*
          };

          return write!(f, "{value}");
        }
      }
    }
    .to_string());
}

fn gen_attr_writer(
    schema_type_attribute: &OpenXmlSchemaTypeAttribute,
    gen_context: &GenContext,
    attributes_ident: &Ident,
) -> TokenStream {
    let ResolvedSchemaTypeAttribute {
        field_name_literal_string,
        field_name_ident,
        ..
    } = schema_type_attribute.resolved_schema_type_attribute(gen_context);

    if schema_type_attribute.is_validator_required() {
        quote! {
          #attributes_ident.push_str(&as_xml_attribute(#field_name_literal_string, &quick_xml::escape::escape(self.#field_name_ident.to_string())));
        }
    } else {
        quote! {
          if let Some(#field_name_ident) = &self.#field_name_ident {
            #attributes_ident.push_str(&as_xml_attribute(#field_name_literal_string, &quick_xml::escape::escape(#field_name_ident.to_string())));
          }
        }
    }
}

fn gen_children_match<'a>(
    children: impl Iterator<Item = &'a OpenXmlSchemaTypeChild>,
    child_choice_enum_type: &Type,
    xml_inner_ident: &Ident,
) -> Option<TokenStream> {
    let child_arms =
        children.map(|child| -> TokenStream {
            let child_name_ident = child.as_last_name_ident();

            parse_quote! {
              #child_choice_enum_type::#child_name_ident(child) => #xml_inner_ident.push_str(&child.to_xml_string(false, with_xmlns)),
            }
        }).collect::<Vec<_>>();

    if child_arms.is_empty() {
        return None;
    }

    return Some(quote! {
        for child in &self.children {
            match child {
                #( #child_arms )*
            };
        }
    });
}

fn gen_sequence_flatten_match(
    schema_type: &OpenXmlSchemaType,
    xml_inner_ident: &Ident,
) -> Result<TokenStream, BuildErrorReport> {
    let child_map = schema_type.child_map();
    let mut child_stmt_list: Vec<Stmt> = vec![];

    for schema_type_particle in &schema_type.particle.items {
        let child = child_map.try_get(schema_type_particle.name.as_str())?;
        let child_name_ident = child.as_property_name_ident();

        match schema_type_particle.as_occurrence() {
            Occurrence::Required => {
                child_stmt_list.push(
                    parse_quote! {
                      #xml_inner_ident.push_str(&self.#child_name_ident.to_xml_string(false, with_xmlns));
                    },
                );
            }
            Occurrence::Optional => {
                child_stmt_list.push(parse_quote! {
                  if let Some(#child_name_ident) = &self.#child_name_ident {
                    #xml_inner_ident.push_str(&#child_name_ident.to_xml_string(false, with_xmlns));
                  }
                });
            }
            Occurrence::Repeated => {
                child_stmt_list.push(parse_quote! {
                  for child in &self.#child_name_ident {
                    #xml_inner_ident.push_str(&child.to_xml_string(false, with_xmlns));
                  }
                });
            }
        };
    }

    return Ok(quote! {
      #( #child_stmt_list )*
    });
}

fn gen_inner_writer(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    attributes_ident: &Ident,
    attributes_writer: &mut Vec<TokenStream>,
    xml_inner_ident: &Ident,
    gen_context: &GenContext,
) -> Result<Option<TokenStream>, BuildErrorReport> {
    let (type_base_class, _) = schema_type.split_name();

    let child_choice_enum_type = schema.enum_child_choice_type(schema_type);

    match schema_type.base_class.as_str() {
        "OpenXmlLeafElement" => return Ok(None),
        "OpenXmlLeafTextElement" => {
            return Ok(Some(quote! {
              if let Some(xml_content) = &self.xml_content {
                #xml_inner_ident.push_str(&xml_content.0.to_string());
              }
            }));
        }
        "OpenXmlCompositeElement"
        | "CustomXmlElement"
        | "OpenXmlPartRootElement"
        | "SdtElement" => {
            if schema_type.children.is_empty() {
                return Ok(None);
            }

            if schema_type.is_one_sequence_flatten() {
                return Ok(Some(gen_sequence_flatten_match(
                    schema_type,
                    xml_inner_ident,
                )?));
            };

            return Ok(gen_children_match(
                schema_type.children.iter(),
                &child_choice_enum_type,
                xml_inner_ident,
            ));
        }
        _ if schema_type.is_derived => {
            let base_class_type = gen_context
                .type_name_type_map
                .try_get(format!("{type_base_class}/").as_str())?;

            for attribute in &base_class_type.attributes {
                attributes_writer.push(gen_attr_writer(attribute, gen_context, attributes_ident));
            }

            // Children must be deduped
            let children: HashMap<_, _> =
                HashMap::from_iter(base_class_type.child_iter().chain(schema_type.child_iter()));

            if children.is_empty() {
                if base_class_type.base_class == "OpenXmlLeafTextElement" {
                    return Ok(Some(quote! {
                      if let Some(xml_content) = &self.xml_content {
                        #xml_inner_ident.push_str(&xml_content.0.to_string());
                      }
                    }));
                };

                return Ok(None);
            }

            if schema_type.is_one_sequence_flatten()
                //TODO: Check if its the same without this
                && base_class_type.composite_type == Some(CompositeType::OneSequence)
            {
                return Ok(Some(gen_sequence_flatten_match(
                    schema_type,
                    xml_inner_ident,
                )?));
            };

            return Ok(gen_children_match(
                children.into_values(),
                &child_choice_enum_type,
                xml_inner_ident,
            ));
        }
        _ => panic!(
            "{:?}",
            report!("Unhandled schema type").attach(format!("{schema_type:?}"))
        ),
    }
}
