use heck::ToUpperCamelCase;
use quote::{format_ident, quote};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rootcause::prelude::ResultExt;
use syn::{Field, Ident, ImplItemFn, ItemEnum, ItemImpl, Type, Variant, parse_quote};

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
    let mut contents = String::with_capacity(const { 512 * 1024 });

    if !schema.types.is_empty() {
        contents.push_str("#![allow(irrefutable_let_patterns)]\n");
        contents.push_str("#![allow(non_snake_case)]\n");
    }

    contents.push_str(
        &schema
            .enums
            .par_iter()
            .map(gen_schema_enum)
            .collect::<Vec<_>>()
            .join("\n"),
    );

    contents.push_str(
        &schema
            .types
            .par_iter()
            .map(|schema_type| gen_schema_type(schema, schema_type, gen_context))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
    );

    Ok(contents)
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

fn gen_schema_type(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    let schema_namespace = gen_context
        .uri_namespace_map
        .try_get(schema.target_namespace.as_str())?;

    let resolved_schema_type = schema_type.resolved_schema_type();
    let ResolvedSchemaType {
        schema_attributes,
        schema_ident,
        schema_base_class_full,
        ..
    } = &resolved_schema_type;

    let mut fields: Vec<Field> = vec![];

    let mut child_choice_enum_item_option: Option<ItemEnum> = None;
    let mut child_choice_enum_impl_option: Option<ItemImpl> = None;
    let mut child_choice_enum_impls: Vec<ItemImpl> = vec![];

    fields.extend(gen_schema_field_attributes(
        schema,
        schema_type,
        gen_context,
    )?);

    if schema_type.base_class == "OpenXmlLeafTextElement" {
        let simple_type_name = gen_xml_content_type(&resolved_schema_type, gen_context)?;

        fields.push(parse_quote! {
            pub xml_content: Option<crate::common::XmlContent<#simple_type_name>>
        });
    } else if schema_type.base_class == "OpenXmlLeafElement" {
    } else if schema_type.base_class == "OpenXmlCompositeElement"
        || schema_type.base_class == "CustomXmlElement"
        || schema_type.base_class == "OpenXmlPartRootElement"
        || schema_type.base_class == "SdtElement"
    {
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
                fields.push(parse_quote! {
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
            .try_get(format!("{schema_base_class_full}/").as_str())
            .attach_with(|| format!("{schema_type:#?}"))?;

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
                fields.push(parse_quote! {
                    pub children: Vec<#enum_ident>
                });
                child_choice_enum_item_option = Some(enum_item);
                child_choice_enum_impl_option = Some(enum_impl);
                child_choice_enum_impls = enum_impls;
            }
        }

        if schema_type.children.is_empty() && base_class_type.base_class == "OpenXmlLeafTextElement"
        {
            let simple_type_name = gen_xml_content_type(&resolved_schema_type, gen_context)?;

            fields.push(parse_quote! {
                pub xml_content: Option<crate::common::XmlContent<#simple_type_name>>
            });
        }
    } else {
        unreachable!("{schema_type:?}");
    }

    return Ok(quote! {
        #( #schema_attributes )*
        #[derive(Clone, Debug, Default)]
        pub struct #schema_ident {
            #( #fields, )*
        }

        #child_choice_enum_item_option

        #child_choice_enum_impl_option

        #( #child_choice_enum_impls )*
    }
    .to_string());
}

fn gen_schema_field_attributes(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    gen_context: &GenContext,
) -> Result<Vec<Field>, BuildErrorReport> {
    let mut fields = Vec::with_capacity(schema_type.attributes.len());

    fn resolved_schema_type_attribute_to_field(
        ResolvedSchemaTypeAttribute {
            field_attributes,
            field_name_ident,
            field_type_wrapped,
            ..
        }: ResolvedSchemaTypeAttribute,
    ) -> Field {
        parse_quote! {
            #( #field_attributes )*
            pub #field_name_ident: #field_type_wrapped
        }
    }

    if schema.needs_xmlns(schema_type) {
        fields.push(parse_quote!(pub xmlns: crate::common::XmlNamespace));
    }

    if schema_type.base_class == "OpenXmlLeafTextElement"
        || schema_type.base_class == "OpenXmlLeafElement"
        || schema_type.base_class == "OpenXmlCompositeElement"
        || schema_type.base_class == "CustomXmlElement"
        || schema_type.base_class == "OpenXmlPartRootElement"
        || schema_type.base_class == "SdtElement"
    {
        for schema_type_attribute in schema_type.attributes.iter() {
            fields.push(resolved_schema_type_attribute_to_field(
                schema_type_attribute.resolved_schema_type_attribute(gen_context),
            ));
        }
    } else if schema_type.is_derived {
        let (type_base_class, _) = schema_type.split_name();

        let base_class_type = gen_context
            .type_name_type_map
            .try_get(format!("{type_base_class}/").as_str())?;

        for schema_type_attribute in schema_type
            .attributes
            .iter()
            .chain(base_class_type.attributes.iter())
        {
            fields.push(resolved_schema_type_attribute_to_field(
                schema_type_attribute.resolved_schema_type_attribute(gen_context),
            ));
        }
    }

    return Ok(fields);
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
    resolved_schema_type: &ResolvedSchemaType,
    gen_context: &GenContext,
) -> Result<Type, BuildErrorReport> {
    let Some(schema_enum) = gen_context
        .enum_type_enum_map
        .get(resolved_schema_type.schema_base_class_full)
    else {
        let r#type = format_ident!(
            "{}",
            simple_type_mapping(resolved_schema_type.schema_base_class_full)
        );
        return Ok(parse_quote!(crate::common::simple_type::#r#type));
    };

    let enum_namespace = gen_context
        .enum_type_namespace_map
        .try_get(schema_enum.r#type.as_str())?;

    return Ok(schema_enum
        .r#type(Some(enum_namespace.prefix.as_str()) == resolved_schema_type.schema_prefix));
}

fn gen_one_sequence_fields(
    schema_type: &OpenXmlSchemaType,
    schema_namespace: &OpenXmlNamespace,
    gen_context: &GenContext,
) -> Result<Vec<Field>, BuildErrorReport> {
    let mut fields = vec![];

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
            Occurrence::Required => fields.push(parse_quote! {
                #[doc = #property_comments]
                pub #child_property_name_ident: std::boxed::Box<#child_variant_type>
            }),
            Occurrence::Optional => fields.push(parse_quote! {
                #[doc = #property_comments]
                pub #child_property_name_ident: Option<std::boxed::Box<#child_variant_type>>
            }),
            Occurrence::Repeated => fields.push(parse_quote! {
                #[doc = #property_comments]
                pub #child_property_name_ident: Vec<#child_variant_type>
            }),
        }
    }

    Ok(fields)
}
