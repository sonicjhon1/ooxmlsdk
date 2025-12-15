use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemEnum, Type, Variant, parse_str, parse2};

use crate::{
    generator::{context::GenContext, simple_type::simple_type_mapping},
    models::{
        OpenXmlNamespace, OpenXmlSchema, OpenXmlSchemaType, OpenXmlSchemaTypeAttribute,
        OpenXmlSchemaTypeChild,
    },
    utils::get_or_panic,
};

pub fn gen_open_xml_schemas(schema: &OpenXmlSchema, gen_context: &GenContext) -> TokenStream {
    let mut token_stream_list: Vec<TokenStream> = vec![];

    let schema_namespace = get_or_panic!(
        gen_context.uri_namespace_map,
        schema.target_namespace.as_str()
    );

    for schema_enum in &schema.enums {
        let e_enum_name_ident: Ident = parse_str(&schema_enum.name.to_upper_camel_case()).unwrap();

        let mut variants: Vec<Variant> = vec![];

        for (i, schema_enum_facet) in schema_enum.facets.iter().enumerate() {
            let variant_ident = schema_enum_facet.as_variant_ident();

            if i == 0 {
                variants.push(
                    parse2(quote! {
                      #[default]
                      #variant_ident
                    })
                    .unwrap(),
                );
            } else {
                variants.push(
                    parse2(quote! {
                      #variant_ident
                    })
                    .unwrap(),
                );
            }
        }

        token_stream_list.push(quote! {
          #[derive(Clone, Debug, Default)]
          pub enum #e_enum_name_ident {
            #( #variants, )*
          }
        })
    }

    for schema_type in &schema.types {
        let mut fields: Vec<TokenStream> = vec![];

        let mut child_choice_enum_option: Option<ItemEnum> = None;

        if schema_type.base_class == "OpenXmlLeafTextElement" {
            for attr in &schema_type.attributes {
                fields.push(gen_attr(attr, schema_namespace, gen_context));
            }

            let simple_type_name = gen_xml_content_type(schema_type, schema_namespace, gen_context);

            fields.push(quote! {
              pub xml_content: Option<#simple_type_name>,
            });
        } else if schema_type.base_class == "OpenXmlLeafElement" {
            for attr in &schema_type.attributes {
                fields.push(gen_attr(attr, schema_namespace, gen_context));
            }
        } else if schema_type.base_class == "OpenXmlCompositeElement"
            || schema_type.base_class == "CustomXmlElement"
            || schema_type.base_class == "OpenXmlPartRootElement"
            || schema_type.base_class == "SdtElement"
        {
            if !schema_type.part.is_empty()
                || schema_type.base_class == "OpenXmlPartRootElement"
                || schema.target_namespace
                    == "http://schemas.openxmlformats.org/drawingml/2006/main"
                || schema.target_namespace
                    == "http://schemas.openxmlformats.org/drawingml/2006/picture"
            {
                fields.push(quote! {
                  pub xmlns: Option<String>,
                });

                fields.push(quote! {
                  pub xmlns_map: std::collections::HashMap<String, String>,
                });

                fields.push(quote! {
                  pub mc_ignorable: Option<String>,
                });
            }

            for attr in &schema_type.attributes {
                fields.push(gen_attr(attr, schema_namespace, gen_context));
            }

            if schema_type.is_one_sequence_flatten() {
                let one_sequence_fields =
                    gen_one_sequence_fields(schema_type, schema_namespace, gen_context);

                fields.extend(one_sequence_fields);
            } else {
                let (field_option, enum_option) = gen_children(
                    &schema_type.class_name,
                    &schema_type.children,
                    schema_namespace,
                    gen_context,
                );

                if let Some(field) = field_option {
                    fields.push(field);
                }

                child_choice_enum_option = enum_option;
            }
        } else if schema_type.is_derived {
            let base_class_type = get_or_panic!(
                gen_context.type_name_type_map,
                &schema_type.name[0..schema_type.name.find('/').unwrap() + 1]
            );

            for attr in &schema_type.attributes {
                fields.push(gen_attr(attr, schema_namespace, gen_context));
            }

            for attr in &base_class_type.attributes {
                fields.push(gen_attr(attr, schema_namespace, gen_context));
            }

            if schema_type.is_one_sequence_flatten()
                && base_class_type.composite_type == "OneSequence"
            {
                let one_sequence_fields =
                    gen_one_sequence_fields(schema_type, schema_namespace, gen_context);

                fields.extend(one_sequence_fields);
            } else {
                let (field_option, enum_option) = gen_children(
                    &schema_type.class_name,
                    &schema_type.children,
                    schema_namespace,
                    gen_context,
                );

                if let Some(field) = field_option {
                    fields.push(field);
                }

                child_choice_enum_option = enum_option;
            }

            if schema_type.children.is_empty()
                && base_class_type.base_class == "OpenXmlLeafTextElement"
            {
                let simple_type_name =
                    gen_xml_content_type(base_class_type, schema_namespace, gen_context);

                fields.push(quote! {
                  pub xml_content: Option<#simple_type_name>,
                });
            }
        } else {
            panic!("{schema_type:?}");
        }

        let struct_name_ident: Ident =
            parse_str(&schema_type.class_name.to_upper_camel_case()).unwrap();

        let summary_doc = format!(" {}", schema_type.summary);

        let version_doc = if schema_type.version.is_empty() {
            " Available in Office2007 and above.".to_string()
        } else {
            format!(" Available in {} and above.", schema_type.version)
        };

        let qualified_doc = if schema_type.name.ends_with('/') {
            " When the object is serialized out as xml, it's qualified name is .".to_string()
        } else {
            format!(
                " When the object is serialized out as xml, it's qualified name is {}.",
                schema_type.split_name().1
            )
        };

        token_stream_list.push(quote! {
          #[doc = #summary_doc]
          #[doc = ""]
          #[doc = #version_doc]
          #[doc = ""]
          #[doc = #qualified_doc]
          #[derive(Clone, Debug, Default)]
          pub struct #struct_name_ident {
            #( #fields )*
          }

          #child_choice_enum_option
        });
    }

    quote! {
      #( #token_stream_list )*
    }
}

fn gen_attr(
    schema: &OpenXmlSchemaTypeAttribute,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> TokenStream {
    let attr_name_ident = schema.as_name_ident();

    let type_ident_raw = if schema.r#type.starts_with("ListValue<") {
        "String".to_string()
    } else if schema.r#type.starts_with("EnumValue<") {
        let (enum_typed_namespace_str, enum_name) = schema.split_type_enum_value_trimmed();
        let enum_name_formatted = enum_name.to_upper_camel_case();

        let enum_prefix = gen_context
            .typed_namespaces
            .iter()
            .find_map(|typed_namespace| {
                if typed_namespace.namespace != enum_typed_namespace_str {
                    return None;
                };

                return gen_context
                    .prefix_schema_map
                    .get(typed_namespace.prefix.as_str())?
                    .enums
                    .iter()
                    .any(|schema_enum| schema_enum.name == enum_name)
                    .then_some(typed_namespace.prefix.as_str());
            })
            .unwrap();

        let enum_namespace = get_or_panic!(gen_context.prefix_namespace_map, enum_prefix);

        if enum_namespace.prefix == schema_namespace.prefix {
            enum_name_formatted
        } else {
            let enum_schema = get_or_panic!(
                gen_context.prefix_schema_map,
                enum_namespace.prefix.as_str()
            );

            format!(
                "crate::schemas::{}::{enum_name_formatted}",
                enum_schema.module_name
            )
        }
    } else {
        format!("crate::schemas::simple_type::{}", &schema.r#type)
    };
    let type_ident: Type = parse_str(&type_ident_raw).unwrap();

    let property_comments_doc = &schema.property_comments;

    let version_doc = if schema.version.is_empty() {
        " Available in Office2007 and above.".to_string()
    } else {
        format!(" Available in {} and above.", schema.version)
    };

    let qualified_doc = format!(
        " Represents the following attribute in the schema: {}",
        schema.as_name_str()
    );

    if schema.is_validator_required() {
        quote! {
          #[doc = #property_comments_doc]
          #[doc = ""]
          #[doc = #version_doc]
          #[doc = ""]
          #[doc = #qualified_doc]
          pub #attr_name_ident: #type_ident,
        }
    } else {
        quote! {
          #[doc = #property_comments_doc]
          #[doc = ""]
          #[doc = #version_doc]
          #[doc = ""]
          #[doc = #qualified_doc]
          pub #attr_name_ident: Option<#type_ident>,
        }
    }
}

fn gen_children(
    class_name: &str,
    children: &Vec<OpenXmlSchemaTypeChild>,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> (Option<TokenStream>, Option<ItemEnum>) {
    if children.is_empty() {
        return (None, None);
    }

    let child_choice_enum_ident: Ident =
        parse_str(&format!("{}ChildChoice", class_name.to_upper_camel_case())).unwrap();

    let field_option = Some(quote! {
      pub children: Vec<#child_choice_enum_ident>,
    });

    let mut variants: Vec<TokenStream> = vec![];

    for child in children {
        let child_type = get_or_panic!(gen_context.type_name_type_map, child.name.as_str());
        let child_namespace =
            get_or_panic!(gen_context.type_name_namespace_map, child.name.as_str());
        let child_schema_name = child_type.class_name.to_upper_camel_case();

        let child_variant_type_raw = if child_namespace.prefix == schema_namespace.prefix {
            child_schema_name
        } else {
            format!(
                "crate::schemas::{}::{child_schema_name}",
                &child_type.module_name
            )
        };
        let child_variant_type: Type = parse_str(&child_variant_type_raw).unwrap();

        let child_variant_name_ident = child.as_last_name_ident();

        variants.push(quote! {
          #child_variant_name_ident(Box<#child_variant_type>),
        });
    }

    let enum_option = Some(
        parse2(quote! {
          #[derive(Clone, Debug)]
          pub enum #child_choice_enum_ident {
            #( #variants )*
          }
        })
        .unwrap(),
    );

    (field_option, enum_option)
}

fn gen_xml_content_type(
    schema_type: &OpenXmlSchemaType,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Type {
    let (first_name, _) = schema_type.split_name();

    let Some(schema_enum) = gen_context.enum_type_enum_map.get(first_name) else {
        return parse_str(&format!(
            "crate::schemas::simple_type::{}",
            simple_type_mapping(first_name)
        ))
        .unwrap();
    };

    let enum_namespace = get_or_panic!(
        gen_context.enum_type_namespace_map,
        schema_enum.r#type.as_str()
    );
    if enum_namespace.prefix == schema_namespace.prefix {
        return parse_str(&schema_enum.name.to_upper_camel_case()).unwrap();
    }

    return parse_str(&format!(
        "crate::schemas::{}::{}",
        &schema_enum.module_name,
        schema_enum.name.to_upper_camel_case()
    ))
    .unwrap();
}

fn gen_one_sequence_fields(
    schema_type: &OpenXmlSchemaType,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Vec<TokenStream> {
    let mut fields: Vec<TokenStream> = vec![];

    let child_map = schema_type.child_map();

    for particle in &schema_type.particle.items {
        let child = get_or_panic!(child_map, particle.name.as_str());
        let child_type = get_or_panic!(gen_context.type_name_type_map, child.name.as_str());
        let child_namespace =
            get_or_panic!(gen_context.type_name_namespace_map, child.name.as_str());
        let child_schema_name = child_type.class_name.to_upper_camel_case();

        let child_variant_type_raw = if child_namespace.prefix == schema_namespace.prefix {
            child_schema_name
        } else {
            format!(
                "crate::schemas::{}::{child_schema_name}",
                &child_type.module_name
            )
        };
        let child_variant_type: Type = parse_str(&child_variant_type_raw).unwrap();

        let child_property_name_ident = child.as_property_name_ident();

        let property_comments = if child.property_comments.is_empty() {
            " _"
        } else {
            &child.property_comments
        };

        if particle.occurs.is_empty() {
            fields.push(quote! {
              #[doc = #property_comments]
              pub #child_property_name_ident: Box<#child_variant_type>,
            });
        } else if particle.occurs[0].min == 0 && particle.occurs[0].max == 1 {
            fields.push(quote! {
              #[doc = #property_comments]
              pub #child_property_name_ident: Option<Box<#child_variant_type>>,
            });
        } else {
            fields.push(quote! {
              #[doc = #property_comments]
              pub #child_property_name_ident: Vec<#child_variant_type>,
            });
        }
    }

    fields
}
