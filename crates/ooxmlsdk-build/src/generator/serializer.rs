use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use rootcause::report;
use syn::{Ident, ImplItemFn, Stmt, Type, parse_quote, parse_str};

use crate::{
    GenContext,
    error::*,
    models::{
        Occurrence, OpenXmlSchema, OpenXmlSchemaType, OpenXmlSchemaTypeAttribute,
        OpenXmlSchemaTypeChild,
    },
    utils::{HashMapOpsError, gen_use_common_glob},
};

pub fn gen_serializer(
    schema: &OpenXmlSchema,
    gen_context: &GenContext,
) -> Result<TokenStream, BuildErrorReport> {
    let mut token_stream_list: Vec<TokenStream> = Vec::with_capacity(schema.types.len() + schema.enums.len());

    if !schema.types.is_empty() {
        token_stream_list.push(gen_use_common_glob());
    }

    for schema_type in &schema.types {
        if schema_type.is_abstract {
            continue;
        }

        let struct_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema.module_name,
            schema_type.class_name.to_upper_camel_case()
        ))
        .unwrap();

        let (_, type_prefixed_name) = schema_type.split_name();
        let (_, type_name) = schema_type.split_last_name();

        let attributes_ident = parse_quote!(attributes);
        let mut xml_tag_attributes_inner: Vec<TokenStream> = vec![];
        for attribute in &schema_type.attributes {
            xml_tag_attributes_inner.push(gen_attr(attribute, &attributes_ident));
        }

        let xml_inner_ident = parse_quote!(xml);
        let xml_inner_writer = gen_inner_writer(
            schema,
            schema_type,
            &attributes_ident,
            &mut xml_tag_attributes_inner,
            &xml_inner_ident,
            gen_context,
        )?;

        // TODO: Is this needed?
        // let xml_needs_header =
        //     !schema_type.part.is_empty() || schema_type.base_class == "OpenXmlPartRootElement";

        let xml_tag_attributes_xmlns_inner: Option<TokenStream> = if !schema_type.part.is_empty()
            || schema_type.base_class == "OpenXmlPartRootElement"
            || ((schema_type.base_class == "OpenXmlCompositeElement"
                || schema_type.base_class == "CustomXmlElement"
                || schema_type.base_class == "OpenXmlPartRootElement"
                || schema_type.base_class == "SdtElement")
                && (schema.target_namespace
                    == "http://schemas.openxmlformats.org/drawingml/2006/main"
                    || schema.target_namespace
                        == "http://schemas.openxmlformats.org/drawingml/2006/picture"))
        {
            Some(quote! {
              if needs_xmlns && let Some(xmlns) = &self.xmlns {
                #attributes_ident.push_str(&as_xml_attribute("xmlns", xmlns));
              }

              for (key, value) in &self.xmlns_map {
                #attributes_ident.push_str(&as_xml_attribute(&format!("xmlns:{key}"), value));
              }

              if let Some(mc_ignorable) = &self.mc_ignorable {
                //TODO: Check if it should be Ignorable or ignorable
                #attributes_ident.push_str(&as_xml_attribute("mc:Ignorable", mc_ignorable));
              }
            })
        } else {
            None
        };

        let xml_tag_attributes: ImplItemFn =
            if xml_tag_attributes_xmlns_inner.is_some() || !xml_tag_attributes_inner.is_empty() {
                parse_quote! {
                  #[allow(unused_variables)]
                  fn xml_tag_attributes(&self, needs_xmlns: bool) -> Option<String> {
                      let mut #attributes_ident = String::with_capacity(
                        const { "xmlns".len() + "xmlns:".len() + "mc:ignorable".len() + 64 },
                      );

                      #xml_tag_attributes_xmlns_inner

                      #( #xml_tag_attributes_inner )*

                      return Some(#attributes_ident);
                  }
                }
            } else {
                parse_quote! {
                  fn xml_tag_attributes(&self, _needs_xmlns: bool) -> Option<String> {
                      return None;
                  }
                }
            };

        let xml_inner: ImplItemFn = if xml_inner_writer.is_some() {
            parse_quote!(
                #[allow(unused_variables)]
                fn xml_inner(&self, with_xmlns: bool) -> Option<String> {
                    let mut #xml_inner_ident = String::with_capacity(512);

                    #xml_inner_writer

                    return Some(#xml_inner_ident);
                }
            )
        } else {
            parse_quote! {
                fn xml_inner(&self, _with_xmlns: bool) -> Option<String> {
                    return None;
                }
            }
        };

        token_stream_list.push(parse_quote!(
          impl Serializeable for #struct_type {
              const PREFIXED_NAME: &str = #type_prefixed_name;

              const NAME: &str = #type_name;

              #xml_tag_attributes

              #xml_inner
          }
        ));
    }

    for schema_enum in &schema.enums {
        let enum_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema.module_name,
            schema_enum.name.to_upper_camel_case()
        ))
        .unwrap();

        let variants = schema_enum.facets.iter().map(|schema_enum_facet| {
            let variant_ident = schema_enum_facet.as_variant_ident();
            let variant_value = &schema_enum_facet.value;

            return quote! {
              Self::#variant_ident => #variant_value,
            };
        });

        token_stream_list.push(parse_quote! {
          impl std::fmt::Display for #enum_type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
              let value = match self {
                #( #variants )*
              };

              return write!(f, "{value}");
            }
          }
        });
    }

    Ok(quote! {
      #( #token_stream_list )*
    })
}

fn gen_attr(schema: &OpenXmlSchemaTypeAttribute, attributes_ident: &Ident) -> TokenStream {
    let attr_value_ident = schema.as_name_ident();
    let attr_name_str = schema.as_name_str();

    if schema.is_validator_required() {
        quote! {
          #attributes_ident.push_str(&as_xml_attribute(#attr_name_str, &quick_xml::escape::escape(self.#attr_value_ident.to_string())));
        }
    } else {
        quote! {
          if let Some(#attr_value_ident) = &self.#attr_value_ident {
            #attributes_ident.push_str(&as_xml_attribute(#attr_name_str, &quick_xml::escape::escape(#attr_value_ident.to_string())));
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

    let child_choice_enum_type: Type = parse_str(&format!(
        "crate::schemas::{}::{}ChildChoice",
        &schema.module_name,
        schema_type.class_name.to_upper_camel_case()
    ))
    .map_err(BuildError::from)?;

    match schema_type.base_class.as_str() {
        "OpenXmlLeafElement" => return Ok(None),
        "OpenXmlLeafTextElement" => {
            return Ok(Some(quote! {
              if let Some(xml_content) = &self.xml_content {
                #xml_inner_ident.push_str(&quick_xml::escape::escape(xml_content.to_string()));
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
                attributes_writer.push(gen_attr(attribute, attributes_ident));
            }

            let mut children = schema_type
                .children
                .iter()
                .chain(base_class_type.children.iter())
                .peekable();

            if children.peek().is_some() {
                if base_class_type.base_class == "OpenXmlLeafTextElement" {
                    return Ok(Some(quote! {
                      if let Some(xml_content) = &self.xml_content {
                        #xml_inner_ident.push_str(&quick_xml::escape::escape(xml_content.to_string()));
                      }
                    }));
                }

                return Ok(None);
            }

            if schema_type.is_one_sequence_flatten()
                //TODO: Check if its the same without this
                && base_class_type.composite_type == "OneSequence"
            {
                return Ok(Some(gen_sequence_flatten_match(
                    schema_type,
                    xml_inner_ident,
                )?));
            };

            return Ok(gen_children_match(
                children,
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
