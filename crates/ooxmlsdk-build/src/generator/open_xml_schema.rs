use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use syn::{Ident, ImplItemFn, ItemEnum, ItemImpl, Type, Variant, parse_quote, parse_str};

use crate::{
    error::*,
    generator::{context::GenContext, simple_type::simple_type_mapping},
    models::*,
    utils::HashMapOpsError,
};

pub fn gen_open_xml_schemas(
    schema: &OpenXmlSchema,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    let mut contents = String::with_capacity(const { 256 * 1024 });

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
            .collect::<Vec<_>>()
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

    let mut child_choice_enum_item_option: Option<ItemEnum> = None;
    let mut child_choice_enum_impl_option: Option<ItemImpl> = None;
    let mut child_choice_enum_impls: Vec<ItemImpl> = vec![];

    if schema_type.base_class == "OpenXmlLeafTextElement" {
        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, gen_context)?);
        }

        let simple_type_name = gen_xml_content_type(schema_type, schema_namespace, gen_context)?;

        fields.push(quote! {
            pub xml_content: Option<#simple_type_name>,
        });
    } else if schema_type.base_class == "OpenXmlLeafElement" {
        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, gen_context)?);
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
                pub xmlns_map: std::collections::BTreeMap<String, String>,
            });

            fields.push(quote! {
                pub mc_ignorable: Option<String>,
            });
        }

        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, gen_context)?);
        }

        if schema_type.is_one_sequence_flatten() {
            let one_sequence_fields =
                gen_one_sequence_fields(schema_type, schema_namespace, gen_context)?;

            fields.extend(one_sequence_fields);
        } else {
            let children = gen_children_enum(
                &schema_type.class_name,
                &schema_type.children,
                schema_namespace,
                gen_context,
            )?;

            if let Some((enum_ident, enum_item, enum_impl, enum_impls)) = children {
                fields.push(quote! {
                    pub children: Vec<#enum_ident>
                });
                child_choice_enum_item_option = Some(enum_item);
                child_choice_enum_impl_option = Some(enum_impl);
                child_choice_enum_impls = enum_impls;
            }
        }
    } else if schema_type.is_derived {
        let base_class_type = gen_context
            .type_name_type_map
            .try_get(format!("{type_base_class}/").as_str())?;

        for attr in &schema_type.attributes {
            fields.push(gen_attr(attr, gen_context)?);
        }

        for attr in &base_class_type.attributes {
            fields.push(gen_attr(attr, gen_context)?);
        }

        if schema_type.is_one_sequence_flatten()
            && base_class_type.composite_type == Some(CompositeType::OneSequence)
        {
            let one_sequence_fields =
                gen_one_sequence_fields(schema_type, schema_namespace, gen_context)?;

            fields.extend(one_sequence_fields);
        } else {
            let children = gen_children_enum(
                &schema_type.class_name,
                &schema_type.children,
                schema_namespace,
                gen_context,
            )?;

            if let Some((enum_ident, enum_item, enum_impl, enum_impls)) = children {
                fields.push(quote! {
                    pub children: Vec<#enum_ident>
                });
                child_choice_enum_item_option = Some(enum_item);
                child_choice_enum_impl_option = Some(enum_impl);
                child_choice_enum_impls = enum_impls;
            }
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

    let struct_name_ident = format_ident!("{}", schema_type.class_name.to_upper_camel_case());

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
            " When the object is serialized out as xml, it's qualified name is ({type_prefixed_name}).",
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

        #child_choice_enum_item_option

        #child_choice_enum_impl_option

        #( #child_choice_enum_impls )*
    }
    .to_string());
}

fn gen_schema_enum(schema_enum: &OpenXmlSchemaEnum) -> String {
    let enum_name_ident = format_ident!("{}", schema_enum.name.to_upper_camel_case());

    let mut variants: Vec<Variant> = vec![];

    for (i, schema_enum_facet) in schema_enum.facets.iter().enumerate() {
        let variant_ident = schema_enum_facet.as_variant_ident();

        if i == 0 {
            variants.push(parse_quote! {
                #[default]
                #variant_ident
            });
        } else {
            variants.push(parse_quote! {
                #variant_ident
            });
        }
    }

    return quote! {
        #[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
        pub enum #enum_name_ident {
            #( #variants, )*
        }
    }
    .to_string();
}

fn gen_attr(
    attr_schema: &OpenXmlSchemaTypeAttribute,
    gen_context: &GenContext,
) -> Result<TokenStream, BuildErrorReport> {
    let attr_name_ident = attr_schema.as_name_ident();
    let attr_type = attr_schema.r#type(gen_context)?;
    let property_comments_doc = &attr_schema.property_comments;

    let version_doc = if attr_schema.version.is_empty() {
        " Available in Office2007 and above.".to_string()
    } else {
        format!(" Available in {} and above.", attr_schema.version)
    };

    let qualified_doc = format!(
        " Represents the following attribute in the schema: ({})",
        attr_schema.as_name_str()
    );

    Ok(if attr_schema.is_validator_required() {
        quote! {
            #[doc = #property_comments_doc]
            #[doc = ""]
            #[doc = #version_doc]
            #[doc = ""]
            #[doc = #qualified_doc]
            pub #attr_name_ident: #attr_type,
        }
    } else {
        quote! {
            #[doc = #property_comments_doc]
            #[doc = ""]
            #[doc = #version_doc]
            #[doc = ""]
            #[doc = #qualified_doc]
            pub #attr_name_ident: Option<#attr_type>,
        }
    })
}

fn gen_children_variant_idents(
    children: &[OpenXmlSchemaTypeChild],
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<Vec<(Ident, Type)>, BuildErrorReport> {
    let idents = children
        .iter()
        .map(|child| {
            let child_type = gen_context
                .type_name_type_map
                .try_get(child.name.as_str())?;
            let child_namespace = gen_context
                .type_name_namespace_map
                .try_get(child.name.as_str())?;

            let child_variant_name_ident = child.as_last_name_ident();
            let child_variant_type =
                child_type.r#type(child_namespace.prefix == schema_namespace.prefix);

            return Ok((child_variant_name_ident, child_variant_type));
        })
        .collect::<Result<Vec<_>, BuildErrorReport>>()?;

    return Ok(idents);
}

fn gen_children_enum(
    class_name: &str,
    children: &[OpenXmlSchemaTypeChild],
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<Option<(Ident, ItemEnum, ItemImpl, Vec<ItemImpl>)>, BuildErrorReport> {
    if children.is_empty() {
        return Ok(None);
    }

    let enum_ident = format_ident!("{}ChildChoice", class_name.to_upper_camel_case());

    let mut enum_variants: Vec<Variant> = Vec::with_capacity(children.len());
    let mut enum_methods: Vec<ImplItemFn> = Vec::with_capacity(children.len());
    let mut enum_impls: Vec<ItemImpl> = Vec::with_capacity(children.len() * 2);

    for (variant_ident, variant_type) in
        gen_children_variant_idents(children, schema_namespace, gen_context)?
    {
        enum_variants.push(parse_quote! {
            #variant_ident(std::boxed::Box<#variant_type>)
        });

        let is_variant_fn_ident = format_ident!("is_{variant_ident}");
        let try_as_fn_ident = format_ident!("try_as_{variant_ident}");
        let try_as_ref_fn_ident = format_ident!("try_as_ref_{variant_ident}");
        let try_as_mut_fn_ident = format_ident!("try_as_mut_{variant_ident}");

        enum_methods.extend([
            parse_quote! {
                pub fn #is_variant_fn_ident(&self) -> bool {
                    matches!(self, Self::#variant_ident(..))
                }
            },
            parse_quote! {
                pub fn #try_as_fn_ident(self) -> Option<std::boxed::Box<#variant_type>> {
                    if let Self::#variant_ident(v) = self {
                        Some(v)
                    } else {
                        None
                    }
                }
            },
            parse_quote! {
                pub fn #try_as_ref_fn_ident(&self) -> Option<&std::boxed::Box<#variant_type>> {
                    if let Self::#variant_ident(v) = self {
                        Some(v)
                    } else {
                        None
                    }
                }
            },
            parse_quote! {
                pub fn #try_as_mut_fn_ident(&mut self) -> Option<&mut std::boxed::Box<#variant_type>> {
                    if let Self::#variant_ident(v) = self {
                        Some(v)
                    } else {
                        None
                    }
                }
            },
        ]);

        enum_impls.push(parse_quote!(
            impl From<#variant_type> for #enum_ident {
                fn from(v: #variant_type) -> Self {
                    Self::#variant_ident(std::boxed::Box::new(v))
                }
            }
        ));
    }

    let enum_item = parse_quote! {
        #[derive(Clone, Debug)]
        pub enum #enum_ident {
            #( #enum_variants, )*
        }
    };

    let enum_impl = parse_quote! {
        impl #enum_ident {
            #( #enum_methods )*
        }
    };

    Ok(Some((enum_ident, enum_item, enum_impl, enum_impls)))
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

    return Ok(schema_enum.r#type(enum_namespace.prefix == schema_namespace.prefix));
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

        let child_property_name_ident = child.as_property_name_ident();
        let child_variant_type =
            child_type.r#type(child_namespace.prefix == schema_namespace.prefix);

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
