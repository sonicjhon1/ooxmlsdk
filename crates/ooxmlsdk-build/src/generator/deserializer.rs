use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::{Arm, Ident, ItemFn, ItemImpl, LitByteStr, Stmt, Type, parse_str, parse2};

use crate::{
    error::*,
    generator::{context::GenContext, simple_type::simple_type_mapping},
    models::{
        Occurrence, OpenXmlSchema, OpenXmlSchemaTypeAttribute, OpenXmlSchemaTypeChild,
        OpenXmlSchemaTypeParticle,
    },
    utils::HashMapOpsError,
};

pub fn gen_deserializers(
    schema: &OpenXmlSchema,
    gen_context: &GenContext,
) -> Result<TokenStream, BuildErrorReport> {
    let mut token_stream_list: Vec<ItemImpl> = vec![];

    let schema_namespace = gen_context
        .uri_namespace_map
        .try_get(schema.target_namespace.as_str())?;

    for schema_enum in &schema.enums {
        let enum_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema.module_name,
            schema_enum.name.to_upper_camel_case()
        ))
        .unwrap();

        let mut variants: Vec<Arm> = vec![];
        let mut byte_variants: Vec<Arm> = vec![];

        for schema_enum_facet in &schema_enum.facets {
            let variant_ident = schema_enum_facet.as_variant_ident();
            let variant_value = &schema_enum_facet.value;

            let variant_value_literal: LitByteStr =
                parse_str(&format!("b\"{variant_value}\"")).unwrap();

            variants.push(
                parse2(quote! {
                  #variant_value => Ok(Self::#variant_ident),
                })
                .unwrap(),
            );

            byte_variants.push(
                parse2(quote! {
                  #variant_value_literal => Ok(Self::#variant_ident),
                })
                .unwrap(),
            );
        }

        token_stream_list.push(
            parse2(quote! {
              impl std::str::FromStr for #enum_type {
                type Err = crate::common::SdkErrorReport;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                  match s {
                    #( #variants )*
                    _ => Err(crate::common::SdkError::CommonError(s.to_string()))?,
                  }
                }
              }
            })
            .unwrap(),
        );

        token_stream_list.push(
            parse2(quote! {
              impl #enum_type {
                pub fn from_bytes(b: &[u8]) -> Result<Self, crate::common::SdkErrorReport> {
                  match b {
                    #( #byte_variants )*
                    other => Err(crate::common::SdkError::CommonError(
                      String::from_utf8_lossy(other).into_owned(),
                    ))?,
                  }
                }
              }
            })
            .unwrap(),
        );
    }

    for schema_type in &schema.types {
        if schema_type.is_abstract {
            continue;
        }

        let schema_class_name_formatted = schema_type.class_name.to_upper_camel_case();

        let struct_type: Type = parse_str(&format!(
            "crate::schemas::{}::{schema_class_name_formatted}",
            &schema.module_name
        ))
        .unwrap();

        let (type_base_class, type_prefixed_name) = schema_type.split_name();
        let (_, type_name_str) = schema_type.split_last_name();

        let type_prefixed_name_literal: LitByteStr =
            parse_str(&format!("b\"{type_prefixed_name}\"")).unwrap();
        let type_name_literal: LitByteStr = parse_str(&format!("b\"{type_name_str}\"")).unwrap();

        let mut field_declaration_list: Vec<Stmt> = vec![];
        let mut attr_match_list: Vec<Arm> = vec![];
        let mut field_unwrap_list: Vec<Stmt> = vec![];
        let mut field_ident_list: Vec<Ident> = vec![];
        let mut loop_declaration_list: Vec<Stmt> = vec![];
        let mut loop_children_stmt_opt: Option<Stmt> = None;
        let mut loop_match_arm_list: Vec<Arm> = vec![];

        let mut loop_children_match_list: Vec<Arm> = vec![];
        let mut loop_children_suffix_match_set: HashSet<String> = HashSet::new();

        let mut attributes: Vec<&OpenXmlSchemaTypeAttribute> = vec![];

        let child_map = schema_type.child_map();

        if schema_type.base_class == "OpenXmlLeafTextElement" {
            for attr in &schema_type.attributes {
                attributes.push(attr);
            }

            field_declaration_list.push(
                parse2(quote! {
                  let mut xml_content = None;
                })
                .unwrap(),
            );

            field_ident_list.push(
                parse2(quote! {
                  xml_content
                })
                .unwrap(),
            );

            loop_match_arm_list.push(gen_simple_child_match_arm(type_base_class, gen_context)?);
        } else if schema_type.base_class == "OpenXmlLeafElement" {
            for attr in &schema_type.attributes {
                attributes.push(attr);
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
                field_declaration_list.push(
                    parse2(quote! {
                      let mut xmlns = None;
                    })
                    .unwrap(),
                );

                field_declaration_list.push(
                    parse2(quote! {
                      let mut xmlns_map = std::collections::HashMap::<String, String>::new();
                    })
                    .unwrap(),
                );

                field_declaration_list.push(
                    parse2(quote! {
                      let mut mc_ignorable = None;
                    })
                    .unwrap(),
                );

                field_ident_list.push(parse_str("xmlns").unwrap());
                field_ident_list.push(parse_str("xmlns_map").unwrap());
                field_ident_list.push(parse_str("mc_ignorable").unwrap());
            }

            for attr in &schema_type.attributes {
                attributes.push(attr);
            }

            let child_choice_enum_type: Type = parse_str(&format!(
                "crate::schemas::{}::{schema_class_name_formatted}ChildChoice",
                &schema.module_name,
            ))
            .unwrap();

            if schema_type.is_one_sequence_flatten() {
                for schema_type_particle in &schema_type.particle.items {
                    let child = child_map.try_get(schema_type_particle.name.as_str())?;

                    let child_property_name_str = child.as_property_name_str();
                    let child_property_name_ident = child.as_property_name_ident();

                    match schema_type_particle.as_occurrence() {
                        Occurrence::Required => {
                            field_declaration_list.push(
                                parse2(quote! {
                                    let mut #child_property_name_ident = None;
                                })
                                .unwrap(),
                            );

                            field_unwrap_list.push(
                                parse2(quote! {
                                    let #child_property_name_ident = #child_property_name_ident
                                        .ok_or_else(|| crate::common::SdkError::CommonError(#child_property_name_str.to_string()))?;
                                })
                                .unwrap(),
                            );
                        }
                        Occurrence::Optional => {
                            field_declaration_list.push(
                                parse2(quote! {
                                  let mut #child_property_name_ident = None;
                                })
                                .unwrap(),
                            );
                        }
                        Occurrence::Repeated => {
                            field_declaration_list.push(
                                parse2(quote! {
                                  let mut #child_property_name_ident = vec![];
                                })
                                .unwrap(),
                            );
                        }
                    };

                    field_ident_list.push(child_property_name_ident);

                    loop_children_match_list.push(gen_one_sequence_match_arm(
                        schema_type_particle,
                        child,
                        gen_context,
                        &mut loop_children_suffix_match_set,
                    )?);
                }
            } else {
                if !schema_type.children.is_empty() {
                    field_declaration_list.push(
                        parse2(quote! {
                          let mut children = vec![];
                        })
                        .unwrap(),
                    );

                    field_ident_list.push(
                        parse2(quote! {
                          children
                        })
                        .unwrap(),
                    );
                }

                for child in &schema_type.children {
                    loop_children_match_list.push(gen_child_match_arm(
                        child,
                        &child_choice_enum_type,
                        gen_context,
                        &mut loop_children_suffix_match_set,
                    )?);
                }
            }
        } else if schema_type.is_derived {
            let base_class_type = gen_context
                .type_name_type_map
                .try_get(format!("{type_base_class}/").as_str())?;

            for attr in &schema_type.attributes {
                attributes.push(attr);
            }

            for attr in &base_class_type.attributes {
                attributes.push(attr);
            }

            if schema_type.is_one_sequence_flatten()
                && base_class_type.composite_type == "OneSequence"
            {
                for schema_type_particle in &schema_type.particle.items {
                    let child = child_map.try_get(schema_type_particle.name.as_str())?;

                    let child_property_name_str = child.as_property_name_str();
                    let child_property_name_ident = child.as_property_name_ident();

                    match schema_type_particle.as_occurrence() {
                        Occurrence::Required => {
                            field_declaration_list.push(
                                parse2(quote! {
                                    let mut #child_property_name_ident = None;
                                })
                                .unwrap(),
                            );

                            field_unwrap_list.push(
                                parse2(quote! {
                                    let #child_property_name_ident = #child_property_name_ident
                                        .ok_or_else(|| crate::common::SdkError::CommonError(#child_property_name_str.to_string()))?;
                                })
                            .unwrap(),
                        );
                        }
                        Occurrence::Optional => {
                            field_declaration_list.push(
                                parse2(quote! {
                                    let mut #child_property_name_ident = None;
                                })
                                .unwrap(),
                            );
                        }
                        Occurrence::Repeated => {
                            field_declaration_list.push(
                                parse2(quote! {
                                    let mut #child_property_name_ident = vec![];
                                })
                                .unwrap(),
                            );
                        }
                    }

                    field_ident_list.push(child_property_name_ident);
                }
            } else if !schema_type.children.is_empty() {
                field_declaration_list.push(
                    parse2(quote! {
                      let mut children = vec![];
                    })
                    .unwrap(),
                );

                field_ident_list.push(
                    parse2(quote! {
                      children
                    })
                    .unwrap(),
                );
            } else if base_class_type.base_class == "OpenXmlLeafTextElement" {
                field_declaration_list.push(
                    parse2(quote! {
                      let mut xml_content = None;
                    })
                    .unwrap(),
                );

                field_ident_list.push(
                    parse2(quote! {
                      xml_content
                    })
                    .unwrap(),
                );
            }

            let child_choice_enum_type: Type = parse_str(&format!(
                "crate::schemas::{}::{schema_class_name_formatted}ChildChoice",
                &schema.module_name,
            ))
            .unwrap();

            if schema_type.is_one_sequence_flatten()
                && base_class_type.composite_type == "OneSequence"
            {
                for schema_type_particle in &schema_type.particle.items {
                    let child = child_map.try_get(schema_type_particle.name.as_str())?;

                    loop_children_match_list.push(gen_one_sequence_match_arm(
                        schema_type_particle,
                        child,
                        gen_context,
                        &mut loop_children_suffix_match_set,
                    )?);
                }
            } else {
                for child in &schema_type.children {
                    loop_children_match_list.push(gen_child_match_arm(
                        child,
                        &child_choice_enum_type,
                        gen_context,
                        &mut loop_children_suffix_match_set,
                    )?);
                }
            }

            if schema_type.children.is_empty()
                && base_class_type.base_class == "OpenXmlLeafTextElement"
            {
                let base_first_name = base_class_type.split_name().0;

                loop_match_arm_list.push(gen_simple_child_match_arm(base_first_name, gen_context)?);
            }
        } else {
            panic!("{schema_type:?}");
        };

        for attr in &attributes {
            let attr_name_str = attr.as_name_str();
            let attr_name_ident = attr.as_name_ident();

            field_declaration_list.push(
                parse2(quote! {
                  let mut #attr_name_ident = None;
                })
                .unwrap(),
            );

            attr_match_list.push(gen_field_match_arm(attr, gen_context)?);

            if attr.is_validator_required() {
                field_unwrap_list.push(
                    parse2(quote! {
                        let #attr_name_ident = #attr_name_ident
                          .ok_or_else(|| crate::common::SdkError::CommonError(#attr_name_str.to_string()))?;
                    })
                    .unwrap(),
                )
            }

            field_ident_list.push(attr_name_ident);
        }

        let mut expect_event_start_stmt: Stmt = parse2(quote! {
            let (e, empty_tag) =
                crate::common::expect_event_start(xml_reader, xml_event, #type_prefixed_name_literal, #type_name_literal)?;
        }).unwrap();

        let attr_match_stmt_opt: Option<Stmt> = if (schema_type.base_class
            == "OpenXmlCompositeElement"
            || schema_type.base_class == "CustomXmlElement"
            || schema_type.base_class == "OpenXmlPartRootElement"
            || schema_type.base_class == "SdtElement")
            && (!schema_type.part.is_empty()
                || schema_type.base_class == "OpenXmlPartRootElement"
                || schema_namespace.uri == "http://schemas.openxmlformats.org/drawingml/2006/main"
                || schema_namespace.uri
                    == "http://schemas.openxmlformats.org/drawingml/2006/picture")
        {
            Some(
                parse2(quote! {
                    for attr in e.attributes().with_checks(false) {
                        let attr = attr.map_err(crate::common::SdkError::from)?;

                        match attr.key.as_ref() {
                            #( #attr_match_list )*
                            b"xmlns" => {
                                xmlns = Some(attr.decode_and_unescape_value(xml_reader.decoder()).map_err(crate::common::SdkError::from)?.into_owned());
                            }
                            b"mc:Ignorable" => {
                                mc_ignorable = Some(attr.decode_and_unescape_value(xml_reader.decoder()).map_err(crate::common::SdkError::from)?.into_owned());
                            }
                            key => {
                                if let Some(xmlns_key) = key.strip_prefix(b"xmlsns:") {
                                    xmlns_map.insert(
                                        String::from_utf8_lossy(xmlns_key).to_string(),
                                        attr.decode_and_unescape_value(xml_reader.decoder()).map_err(crate::common::SdkError::from)?.into_owned(),
                                    );
                                }
                            }
                        }
                    }
                })
                .unwrap(),
            )
        } else if !attr_match_list.is_empty() {
            Some(
                parse2(quote! {
                  for attr in e.attributes().with_checks(false) {
                    let attr = attr.map_err(crate::common::SdkError::from)?;

                    #[allow(clippy::single_match)]
                    match attr.key.as_ref() {
                      #( #attr_match_list )*
                      _ => {}
                    }
                  }
                })
                .unwrap(),
            )
        } else {
            expect_event_start_stmt = parse2(quote! {
              let (_, empty_tag) =
                crate::common::expect_event_start(xml_reader, xml_event, #type_prefixed_name_literal, #type_name_literal)?;
            }).unwrap();

            None
        };

        if !loop_children_match_list.is_empty() {
            loop_declaration_list.push(
                parse2(quote! {
                  let mut e_opt = None;
                })
                .unwrap(),
            );

            loop_declaration_list.push(
                parse2(quote! {
                  let mut e_empty = false;
                })
                .unwrap(),
            );

            loop_match_arm_list.push(
                parse2(quote! {
                  quick_xml::events::Event::Start(e) => {
                    e_opt = Some(e);
                  }
                })
                .unwrap(),
            );

            loop_match_arm_list.push(
                parse2(quote! {
                  quick_xml::events::Event::Empty(e) => {
                    e_empty = true;
                    e_opt = Some(e);
                  }
                })
                .unwrap(),
            );

            loop_children_stmt_opt = Some(
                parse2(quote! {
                  if let Some(e) = e_opt {
                    match e.name().as_ref() {
                      #( #loop_children_match_list )*
                      _ => Err(super::super::common::SdkError::CommonError(
                        #schema_class_name_formatted.to_string(),
                      ))?,
                    }
                  }
                })
                .unwrap(),
            )
        }

        let deserialize_inner_fn: ItemFn = parse2(quote! {
          fn deserialize_inner<'de>(
            xml_reader: &mut impl crate::common::XmlReader<'de>,
            xml_event: Option<(quick_xml::events::BytesStart<'de>, bool)>,
          ) -> Result<Self, crate::common::SdkErrorReport> {
            #expect_event_start_stmt

            #( #field_declaration_list )*

            #attr_match_stmt_opt

            if !empty_tag {
              loop {
                #( #loop_declaration_list )*

                match xml_reader.next()? {
                  #( #loop_match_arm_list )*
                  quick_xml::events::Event::End(e) => match e.name().as_ref() {
                    #type_prefixed_name_literal | #type_name_literal => {
                      break;
                    }
                    _ => (),
                  },
                  quick_xml::events::Event::Eof => Err(crate::common::SdkError::UnknownError)?,
                  _ => (),
                }

                #loop_children_stmt_opt
              }
            }

            #( #field_unwrap_list )*

            Ok(Self {
              #( #field_ident_list, )*
            })
          }
        })
        .unwrap();

        token_stream_list.push(
            parse2(quote! {
              impl crate::common::Deserializeable for #struct_type {
                #deserialize_inner_fn
              }
            })
            .unwrap(),
        );
    }

    Ok(quote! {
      #( #token_stream_list )*
    })
}

fn gen_one_sequence_match_arm(
    schema_type_particle: &OpenXmlSchemaTypeParticle,
    child: &OpenXmlSchemaTypeChild,
    gen_context: &GenContext,
    loop_children_suffix_match_set: &mut HashSet<String>,
) -> Result<Arm, BuildErrorReport> {
    let child_type = gen_context
        .type_name_type_map
        .try_get(child.name.as_str())?;

    let (_, child_prefixed_name) = child.split_name();
    let (_, child_name) = child.split_last_name();
    let child_property_name_ident = child.as_property_name_ident();

    let child_prefixed_name_literal: LitByteStr =
        parse_str(&format!("b\"{child_prefixed_name}\"")).map_err(BuildError::from)?;
    let child_name_literal: LitByteStr =
        parse_str(&format!("b\"{child_name}\"")).map_err(BuildError::from)?;

    let child_variant_type: Type = parse_str(&format!(
        "crate::schemas::{}::{}",
        &child_type.module_name,
        child_type.class_name.to_upper_camel_case()
    ))
    .map_err(BuildError::from)?;

    // TODO: Simplify again
    if loop_children_suffix_match_set.insert(child_name.to_string()) {
        match schema_type_particle.as_occurrence() {
            Occurrence::Required | Occurrence::Optional => Ok(parse2(quote! {
                #child_prefixed_name_literal | #child_name_literal => {
                    #child_property_name_ident = Some(std::boxed::Box::new(
                        #child_variant_type::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                    ));
                }
            })
            .map_err(BuildError::from)?),
            Occurrence::Repeated => Ok(parse2(quote! {
                #child_prefixed_name_literal | #child_name_literal => {
                    #child_property_name_ident.push(
                        #child_variant_type::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                    );
                }
            })
            .map_err(BuildError::from)?),
        }
    } else {
        match schema_type_particle.as_occurrence() {
            Occurrence::Required | Occurrence::Optional => Ok(parse2(quote! {
                #child_prefixed_name_literal => {
                    #child_property_name_ident = Some(std::boxed::Box::new(
                        #child_variant_type::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                    ));
                }
            })
            .map_err(BuildError::from)?),
            Occurrence::Repeated => Ok(parse2(quote! {
                #child_prefixed_name_literal => {
                    #child_property_name_ident.push(
                        #child_variant_type::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                    );
                }
            })
            .map_err(BuildError::from)?),
        }
    }
}

fn gen_child_match_arm(
    child: &OpenXmlSchemaTypeChild,
    child_choice_enum_ident: &Type,
    gen_context: &GenContext,
    loop_children_suffix_match_set: &mut HashSet<String>,
) -> Result<Arm, BuildErrorReport> {
    let child_type = gen_context
        .type_name_type_map
        .try_get(child.name.as_str())?;

    let (_, child_prefixed_name) = child.split_name();
    let (_, child_name) = child.split_last_name();

    let child_prefixed_name_literal: LitByteStr =
        parse_str(&format!("b\"{child_prefixed_name}\"")).map_err(BuildError::from)?;
    let child_name_literal: LitByteStr =
        parse_str(&format!("b\"{child_name}\"")).map_err(BuildError::from)?;

    let child_variant_name_ident = child.as_last_name_ident();

    let child_variant_type: Type = parse_str(&format!(
        "crate::schemas::{}::{}",
        &child_type.module_name,
        child_type.class_name.to_upper_camel_case()
    ))
    .map_err(BuildError::from)?;

    if loop_children_suffix_match_set.insert(child_name.to_string()) {
        return Ok(parse2(quote! {
          #child_prefixed_name_literal | #child_name_literal => {
            children.push(#child_choice_enum_ident::#child_variant_name_ident(std::boxed::Box::new(
              #child_variant_type::deserialize_inner(xml_reader, Some((e, e_empty)))?,
            )));
          }
        })
        .map_err(BuildError::from)?);
    };

    return Ok(parse2(quote! {
      #child_prefixed_name_literal => {
        children.push(#child_choice_enum_ident::#child_variant_name_ident(std::boxed::Box::new(
          #child_variant_type::deserialize_inner(xml_reader, Some((e, e_empty)))?,
        )));
      }
    })
    .map_err(BuildError::from)?);
}

fn gen_simple_child_match_arm(
    first_name: &str,
    gen_context: &GenContext,
) -> Result<Arm, BuildErrorReport> {
    if let Some(schema_enum) = gen_context.enum_type_enum_map.get(first_name) {
        let simple_type_name: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema_enum.module_name,
            schema_enum.name.to_upper_camel_case()
        ))
        .map_err(BuildError::from)?;

        return Ok(parse2(quote! {
          quick_xml::events::Event::Text(t) => {
            xml_content = Some(#simple_type_name::from_bytes(&t.into_inner())?);
          }
        })
        .map_err(BuildError::from)?);
    }

    let simple_type_str = simple_type_mapping(first_name);

    let enum_type: Type = parse_str(&format!("crate::common::simple_type::{simple_type_str}"))
        .map_err(BuildError::from)?;

    return Ok(parse2(match simple_type_str {
        "Base64BinaryValue" | "DateTimeValue" | "DecimalValue" | "HexBinaryValue"
        | "IntegerValue" | "SByteValue" | "StringValue" => quote! {
          quick_xml::events::Event::Text(t) => {
            xml_content = Some(t.decode().map_err(crate::common::SdkError::from)?.to_string());
          }
        },
        "BooleanValue" | "OnOffValue" | "TrueFalseBlankValue" | "TrueFalseValue" => quote! {
          quick_xml::events::Event::Text(t) => {
            xml_content = Some(crate::common::parse_bool_bytes(&t.into_inner())?);
          }
        },
        "ByteValue" | "Int16Value" | "Int32Value" | "Int64Value" | "UInt16Value"
        | "UInt32Value" | "UInt64Value" | "DoubleValue" | "SingleValue" => quote! {
          quick_xml::events::Event::Text(t) => {
            xml_content = Some(
              t.decode().map_err(crate::common::SdkError::from)?.parse::<#enum_type>().map_err(crate::common::SdkError::from)?
            );
          }
        },
        _ => unreachable!("{simple_type_str}"),
    })
    .map_err(BuildError::from)?);
}

fn gen_field_match_arm(
    schema: &OpenXmlSchemaTypeAttribute,
    gen_context: &GenContext,
) -> Result<Arm, BuildErrorReport> {
    let attr_name_ident = schema.as_name_ident();
    let attr_name_str = schema.as_name_str();

    let attr_name_literal: LitByteStr =
        parse_str(&format!("b\"{attr_name_str}\"")).map_err(BuildError::from)?;

    Ok(parse2(if schema.r#type.starts_with("ListValue<") {
        quote! {
            #attr_name_literal => {
                #attr_name_ident = Some(attr.decode_and_unescape_value(xml_reader.decoder()).map_err(crate::common::SdkError::from)?.into_owned());
            }
        }
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

        let enum_schema = gen_context.prefix_schema_map.try_get( enum_namespace.prefix.as_str())?;

        let enum_type: Type = parse_str(&format!(
            "crate::schemas::{}::{enum_name_formatted}",
            enum_schema.module_name,
        ))
        .map_err(BuildError::from)?;

        quote! {
          #attr_name_literal => {
            #attr_name_ident = Some(#enum_type::from_bytes(&attr.value)?);
          }
        }
    } else {
        match schema.r#type.as_str() {
          "Base64BinaryValue" | "DateTimeValue" | "DecimalValue" | "HexBinaryValue"
          | "IntegerValue" | "SByteValue" | "StringValue" => quote! {
            #attr_name_literal => {
              #attr_name_ident = Some(attr.decode_and_unescape_value(xml_reader.decoder()).map_err(crate::common::SdkError::from)?.into_owned());
            }
          },
          "BooleanValue" | "OnOffValue" | "TrueFalseBlankValue" | "TrueFalseValue" => quote! {
            #attr_name_literal => {
              #attr_name_ident = Some(crate::common::parse_bool_bytes(&attr.value)?);
            }
          },
          "ByteValue" | "Int16Value" | "Int32Value" | "Int64Value" | "UInt16Value" | "UInt32Value"
          | "UInt64Value" | "DoubleValue" | "SingleValue" => {
            let enum_type: Type =
              parse_str(&format!("crate::common::simple_type::{}", &schema.r#type)).map_err(BuildError::from)?;

            quote! {
              #attr_name_literal => {
                #attr_name_ident = Some(
                  attr
                    .decode_and_unescape_value(xml_reader.decoder()).map_err(crate::common::SdkError::from)?
                    .parse::<#enum_type>().map_err(crate::common::SdkError::from)?,
                );
              }
            }
          }
          _ => panic!("{}", schema.r#type),
        }
    })
    .map_err(BuildError::from)?)
}
