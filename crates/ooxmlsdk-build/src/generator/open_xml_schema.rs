use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use syn::{Ident, ItemEnum, Type, Variant, parse_str, parse2};

use crate::{
    error::*,
    generator::{context::GenContext, simple_type::simple_type_mapping},
    models::{
        Occurrence, OpenXmlNamespace, OpenXmlSchema, OpenXmlSchemaEnum, OpenXmlSchemaType,
        OpenXmlSchemaTypeAttribute, OpenXmlSchemaTypeChild,
    },
    utils::HashMapOpsError,
};

pub fn gen_open_xml_schemas(
    schema: &OpenXmlSchema,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    let mut contents = String::with_capacity(const { 128 * 1024 });

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
            .map(gen_schema_enum)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
    );

    Ok(contents)
}

fn gen_schema_type(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    let schema_namespace = gen_context
        .uri_namespace_map
        .try_get(schema.target_namespace.as_str())?;

    let (type_base_class, type_prefixed_name) = schema_type.split_name();

    let mut fields: Vec<TokenStream> = vec![];

    let mut child_choice_enum_option: Option<ItemEnum> = None;

    if schema_type.base_class == "OpenXmlLeafTextElement" {
        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, schema_namespace, gen_context)?);
        }

        let simple_type_name = gen_xml_content_type(schema_type, schema_namespace, gen_context)?;

        fields.push(quote! {
            pub xml_content: Option<#simple_type_name>,
        });
    } else if schema_type.base_class == "OpenXmlLeafElement" {
        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, schema_namespace, gen_context)?);
        }
    } else if schema_type.base_class == "OpenXmlCompositeElement"
        || schema_type.base_class == "CustomXmlElement"
        || schema_type.base_class == "OpenXmlPartRootElement"
        || schema_type.base_class == "SdtElement"
    {
        if !schema_type.part.is_empty()
            || schema_type.base_class == "OpenXmlPartRootElement"
            || schema.target_namespace == "http://schemas.openxmlformats.org/drawingml/2006/main"
            || schema.target_namespace == "http://schemas.openxmlformats.org/drawingml/2006/picture"
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
            fields.push(gen_attr(attr, schema_namespace, gen_context)?);
        }

        if schema_type.is_one_sequence_flatten() {
            let one_sequence_fields =
                gen_one_sequence_fields(schema_type, schema_namespace, gen_context)?;

            fields.extend(one_sequence_fields);
        } else {
            let (field_option, enum_option) = gen_children(
                &schema_type.class_name,
                &schema_type.children,
                schema_namespace,
                gen_context,
            )?;

            if let Some(field) = field_option {
                fields.push(field);
            }

            child_choice_enum_option = enum_option;
        }
    } else if schema_type.is_derived {
        let base_class_type = gen_context
            .type_name_type_map
            .try_get(format!("{type_base_class}/").as_str())?;

        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, schema_namespace, gen_context)?);
        }

        for attr in &base_class_type.attributes {
            fields.push(gen_attr(attr, schema_namespace, gen_context)?);
        }

        if schema_type.is_one_sequence_flatten() && base_class_type.composite_type == "OneSequence"
        {
            let one_sequence_fields =
                gen_one_sequence_fields(schema_type, schema_namespace, gen_context)?;

            fields.extend(one_sequence_fields);
        } else {
            let (field_option, enum_option) = gen_children(
                &schema_type.class_name,
                &schema_type.children,
                schema_namespace,
                gen_context,
            )?;

            if let Some(field) = field_option {
                fields.push(field);
            }

            child_choice_enum_option = enum_option;
        }

        if schema_type.children.is_empty() && base_class_type.base_class == "OpenXmlLeafTextElement"
        {
            let simple_type_name =
                gen_xml_content_type(base_class_type, schema_namespace, gen_context)?;

            fields.push(quote! {
                pub xml_content: Option<#simple_type_name>,
            });
        }
    } else {
        unreachable!("{schema_type:?}");
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
            " When the object is serialized out as xml, it's qualified name is {type_prefixed_name}.",
        )
    };

    return Ok(quote! {
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
    }
    .to_string());
}

fn gen_schema_enum(schema_enum: &OpenXmlSchemaEnum) -> Result<String, BuildErrorReport> {
    let enum_name_ident: Ident =
        parse_str(&schema_enum.name.to_upper_camel_case()).map_err(BuildError::from)?;

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

    return Ok(quote! {
        #[derive(Clone, Debug, Default)]
        pub enum #enum_name_ident {
            #( #variants, )*
        }
    }
    .to_string());
}

fn gen_attr(
    schema: &OpenXmlSchemaTypeAttribute,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<TokenStream, BuildErrorReport> {
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

        let enum_namespace = gen_context.prefix_namespace_map.try_get(enum_prefix)?;

        if enum_namespace.prefix == schema_namespace.prefix {
            enum_name_formatted
        } else {
            let enum_schema = gen_context
                .prefix_schema_map
                .try_get(enum_namespace.prefix.as_str())?;

            format!(
                "crate::schemas::{}::{enum_name_formatted}",
                enum_schema.module_name
            )
        }
    } else {
        format!("crate::common::simple_type::{}", &schema.r#type)
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

    Ok(if schema.is_validator_required() {
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
    })
}

fn gen_children(
    class_name: &str,
    children: &Vec<OpenXmlSchemaTypeChild>,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<(Option<TokenStream>, Option<ItemEnum>), BuildErrorReport> {
    if children.is_empty() {
        return Ok((None, None));
    }

    let child_choice_enum_ident: Ident =
        parse_str(&format!("{}ChildChoice", class_name.to_upper_camel_case())).unwrap();

    let field_option = Some(quote! {
        pub children: Vec<#child_choice_enum_ident>,
    });

    let mut variants: Vec<TokenStream> = vec![];

    for child in children {
        let child_type = gen_context
            .type_name_type_map
            .try_get(child.name.as_str())?;
        let child_namespace = gen_context
            .type_name_namespace_map
            .try_get(child.name.as_str())?;
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
            #child_variant_name_ident(std::boxed::Box<#child_variant_type>),
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

    Ok((field_option, enum_option))
}

fn gen_xml_content_type(
    schema_type: &OpenXmlSchemaType,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<Type, BuildErrorReport> {
    let (first_name, _) = schema_type.split_name();

    let Some(schema_enum) = gen_context.enum_type_enum_map.get(first_name) else {
        return Ok(parse_str(&format!(
            "crate::common::simple_type::{}",
            simple_type_mapping(first_name)
        ))
        .map_err(BuildError::from)?);
    };

    let enum_namespace = gen_context
        .enum_type_namespace_map
        .try_get(schema_enum.r#type.as_str())?;
    if enum_namespace.prefix == schema_namespace.prefix {
        return Ok(parse_str(&schema_enum.name.to_upper_camel_case()).map_err(BuildError::from)?);
    }

    return Ok(parse_str(&format!(
        "crate::schemas::{}::{}",
        &schema_enum.module_name,
        schema_enum.name.to_upper_camel_case()
    ))
    .map_err(BuildError::from)?);
}

fn gen_one_sequence_fields(
    schema_type: &OpenXmlSchemaType,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<Vec<TokenStream>, BuildErrorReport> {
    let mut fields: Vec<TokenStream> = vec![];

    let child_map = schema_type.child_map();

    for particle in &schema_type.particle.items {
        let child = child_map.try_get(particle.name.as_str())?;
        let child_type = gen_context
            .type_name_type_map
            .try_get(child.name.as_str())?;
        let child_namespace = gen_context
            .type_name_namespace_map
            .try_get(child.name.as_str())?;
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

        match particle.as_occurrence() {
            Occurrence::Required => fields.push(quote! {
                #[doc = #property_comments]
                pub #child_property_name_ident: std::boxed::Box<#child_variant_type>,
            }),
            Occurrence::Optional => fields.push(quote! {
                #[doc = #property_comments]
                pub #child_property_name_ident: Option<std::boxed::Box<#child_variant_type>>,
            }),
            Occurrence::Repeated => fields.push(quote! {
                #[doc = #property_comments]
                pub #child_property_name_ident: Vec<#child_variant_type>,
            }),
        }
    }

    Ok(fields)
}
