use quote::{format_ident, quote};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use syn::{Arm, Ident, ItemFn, LitByteStr, Stmt, Type, parse_quote, parse_str};

use crate::{
    error::*,
    generator::{context::GenContext, simple_type::simple_type_mapping},
    models::*,
    utils::{HashMapOpsError, gen_use_common_glob},
};

pub fn gen_deserializers(
    schema: &OpenXmlSchema,
    gen_context: &GenContext,
) -> Result<String, BuildErrorReport> {
    let mut contents = String::with_capacity(const { 512 * 1024 });

    if !schema.types.is_empty() || !schema.enums.is_empty() {
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
    if schema_type.is_abstract {
        return Ok(String::with_capacity(0));
    }

    let struct_type = schema.struct_type(schema_type);

    let (type_base_class, _) = schema_type.split_name();

    let event_ident = format_ident!("xml_bytes");

    let mut field_declaration_list: Vec<Stmt> = vec![];
    let mut attr_match_list: Vec<Arm> = vec![];
    let mut field_unwrap_list: Vec<Stmt> = vec![];
    let mut field_ident_list: Vec<Ident> = vec![];
    let mut loop_declaration_list: Vec<Stmt> = vec![];
    let mut loop_children_stmt_opt: Option<Stmt> = None;
    let mut loop_match_arm_list: Vec<Arm> = vec![];

    let mut loop_children_match_list: Vec<Arm> = vec![];
    let mut loop_children_ident_set: HashSet<Ident> = HashSet::new();

    let mut attributes: Vec<&OpenXmlSchemaTypeAttribute> = vec![];

    let child_map = schema_type.child_map();

    if schema.needs_xmlns(schema_type) {
        field_declaration_list.push(parse_quote! {
          let mut xmlns = XmlNamespace::default();
        });

        field_ident_list.push(format_ident!("xmlns"));
    }

    if schema_type.base_class == "OpenXmlLeafTextElement" {
        for attr in &schema_type.attributes {
            attributes.push(attr);
        }

        field_declaration_list.push(parse_quote! {
          let mut xml_content = None;
        });

        field_ident_list.push(format_ident!("xml_content"));

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
        for attr in &schema_type.attributes {
            attributes.push(attr);
        }

        if schema_type.is_one_sequence_flatten() {
            for schema_type_particle in &schema_type.particle.items {
                let child = child_map.try_get(schema_type_particle.name.as_str())?;

                let child_property_name_str = child.as_property_name_str();
                let child_property_name_ident = child.as_property_name_ident();

                match schema_type_particle.as_occurrence() {
                    Occurrence::Required => {
                        field_declaration_list.push(parse_quote! {
                            let mut #child_property_name_ident = None;
                        });

                        field_unwrap_list.push(parse_quote! {
                            let #child_property_name_ident = #child_property_name_ident
                                .ok_or_else(|| SdkError::CommonError(#child_property_name_str.to_string()))?;
                        });
                    }
                    Occurrence::Optional => {
                        field_declaration_list.push(parse_quote! {
                            let mut #child_property_name_ident = None;
                        });
                    }
                    Occurrence::Repeated => {
                        field_declaration_list.push(parse_quote! {
                            let mut #child_property_name_ident = vec![];
                        });
                    }
                };

                field_ident_list.push(child_property_name_ident);

                if let Some(arm) = gen_one_sequence_match_arm(
                    &event_ident,
                    schema_type_particle,
                    child,
                    gen_context,
                    &mut loop_children_ident_set,
                )? {
                    loop_children_match_list.push(arm);
                }
            }
        } else {
            if !schema_type.children.is_empty() {
                field_declaration_list.push(parse_quote! {
                  let mut children = vec![];
                });

                field_ident_list.push(parse_quote! {
                  children
                });
            }

            let child_choice_enum_type = schema.enum_child_choice_type(schema_type);

            for child in &schema_type.children {
                if let Some(arm) = gen_child_match_arm(
                    &event_ident,
                    child,
                    &child_choice_enum_type,
                    gen_context,
                    &mut loop_children_ident_set,
                )? {
                    loop_children_match_list.push(arm);
                }
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
            && base_class_type.composite_type == Some(CompositeType::OneSequence)
        {
            for schema_type_particle in &schema_type.particle.items {
                let child = child_map.try_get(schema_type_particle.name.as_str())?;

                let child_property_name_str = child.as_property_name_str();
                let child_property_name_ident = child.as_property_name_ident();

                match schema_type_particle.as_occurrence() {
                    Occurrence::Required => {
                        field_declaration_list.push(parse_quote! {
                            let mut #child_property_name_ident = None;
                        });

                        field_unwrap_list.push(parse_quote! {
                            let #child_property_name_ident = #child_property_name_ident
                                .ok_or_else(|| SdkError::CommonError(#child_property_name_str.to_string()))?;
                        });
                    }
                    Occurrence::Optional => {
                        field_declaration_list.push(parse_quote! {
                            let mut #child_property_name_ident = None;
                        });
                    }
                    Occurrence::Repeated => {
                        field_declaration_list.push(parse_quote! {
                            let mut #child_property_name_ident = vec![];
                        });
                    }
                }

                field_ident_list.push(child_property_name_ident);
            }
        } else if !schema_type.children.is_empty() {
            field_declaration_list.push(parse_quote! {
              let mut children = vec![];
            });

            field_ident_list.push(parse_quote! {
              children
            });
        } else if base_class_type.base_class == "OpenXmlLeafTextElement" {
            field_declaration_list.push(parse_quote! {
              let mut xml_content = None;
            });

            field_ident_list.push(parse_quote! {
              xml_content
            });
        }

        if schema_type.is_one_sequence_flatten()
            && base_class_type.composite_type == Some(CompositeType::OneSequence)
        {
            for schema_type_particle in &schema_type.particle.items {
                let child = child_map.try_get(schema_type_particle.name.as_str())?;

                if let Some(arm) = gen_one_sequence_match_arm(
                    &event_ident,
                    schema_type_particle,
                    child,
                    gen_context,
                    &mut loop_children_ident_set,
                )? {
                    loop_children_match_list.push(arm);
                }
            }
        } else {
            let child_choice_enum_type = schema.enum_child_choice_type(schema_type);

            for child in &schema_type.children {
                if let Some(arm) = gen_child_match_arm(
                    &event_ident,
                    child,
                    &child_choice_enum_type,
                    gen_context,
                    &mut loop_children_ident_set,
                )? {
                    loop_children_match_list.push(arm);
                }
            }
        }

        if schema_type.children.is_empty() && base_class_type.base_class == "OpenXmlLeafTextElement"
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

        field_declaration_list.push(parse_quote! {
            let mut #attr_name_ident = None;
        });

        attr_match_list.push(gen_field_match_arm(attr, gen_context)?);

        if attr.is_validator_required() {
            field_unwrap_list.push(parse_quote! {
                let #attr_name_ident = #attr_name_ident
                    .ok_or_else(|| SdkError::CommonError(#attr_name_str.to_string()))?;
            })
        }

        field_ident_list.push(attr_name_ident);
    }

    let mut expect_event_start_stmt: Stmt = parse_quote! {
        let (#event_ident, empty_tag) = expect_event_start::<Self>(xml_reader, xml_event)?;
    };

    let attr_match_stmt_opt: Option<Stmt> = if schema.needs_xmlns(schema_type) {
        Some(parse_quote! {
            for attr in #event_ident.attributes().with_checks(false) {
                let attr = attr.map_err(SdkError::from)?;
                if xmlns.deserialize_attributes(xml_reader, &attr)?.is_some() {
                    continue;
                }

                match attr.key.0 {
                    #( #attr_match_list )*
                    _ => {}
                }
            }
        })
    } else if !attr_match_list.is_empty() {
        Some(parse_quote! {
            for attr in #event_ident.attributes().with_checks(false) {
                let attr = attr.map_err(SdkError::from)?;

                #[allow(clippy::single_match)]
                match attr.key.0 {
                    #( #attr_match_list )*
                    _ => {}
                }
            }
        })
    } else {
        expect_event_start_stmt = parse_quote! {
            let (_, empty_tag) = expect_event_start::<Self>(xml_reader, xml_event)?;
        };

        None
    };

    if !loop_children_match_list.is_empty() {
        loop_declaration_list.extend([
            parse_quote! {
                let mut e_opt = None;
            },
            parse_quote! {
                let mut e_empty = false;
            },
        ]);

        loop_match_arm_list.extend([
            parse_quote! {
                quick_xml::events::Event::Start(#event_ident) => {
                    e_opt = Some(#event_ident);
                }
            },
            parse_quote! {
                quick_xml::events::Event::Empty(#event_ident) => {
                    e_empty = true;
                    e_opt = Some(#event_ident);
                }
            },
        ]);

        let schema_type_class_name = schema_type.class_name_ident().to_string();

        loop_children_stmt_opt = Some(parse_quote! {
            if let Some(#event_ident) = e_opt {
                match #event_ident.name().0 {
                #( #loop_children_match_list )*
                _ => {
                        tracing::warn!(
                            "Skipping non-matching tag: ({}) from schema: ({})",
                            String::from_utf8_lossy(#event_ident.name().0),
                            #schema_type_class_name
                        );
                        continue;
                    },
                }
            }
        })
    }

    let deserialize_inner_fn: ItemFn = parse_quote! {
      fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(quick_xml::events::BytesStart<'de>, bool)>,
      ) -> Result<Self, SdkErrorReport> {
        #expect_event_start_stmt

        #( #field_declaration_list )*

        #attr_match_stmt_opt

        if !empty_tag {
          loop {
            #( #loop_declaration_list )*

            match xml_reader.next()? {
                #( #loop_match_arm_list )*
                quick_xml::events::Event::End(#event_ident) if Self::matched_name(#event_ident.name().0) => {
                    break;
                },
                quick_xml::events::Event::Eof => Err(SdkError::UnknownError)?,
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
    };

    return Ok(quote! {
      impl Deserializeable for #struct_type {
        #deserialize_inner_fn
      }
    }
    .to_string());
}

fn gen_schema_enum(
    schema: &OpenXmlSchema,
    schema_enum: &OpenXmlSchemaEnum,
) -> Result<String, BuildErrorReport> {
    let enum_type = schema.enum_type(schema_enum);

    let mut variants: Vec<Arm> = Vec::with_capacity(schema_enum.facets.len());
    let mut byte_variants: Vec<Arm> = Vec::with_capacity(schema_enum.facets.len());

    for schema_enum_facet in &schema_enum.facets {
        let variant_ident = schema_enum_facet.as_variant_ident();
        let variant_value = &schema_enum_facet.value;

        variants.push(parse_quote! {
          #variant_value => Ok(Self::#variant_ident),
        });

        let variant_value_literal: LitByteStr =
            parse_str(&format!("b\"{variant_value}\"")).map_err(BuildError::from)?;

        byte_variants.push(parse_quote! {
          #variant_value_literal => Ok(Self::#variant_ident),
        });
    }

    return Ok(quote! {
      impl std::str::FromStr for #enum_type {
        type Err = SdkErrorReport;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
          match s {
            #( #variants )*
            _ => Err(SdkError::CommonError(s.to_string()))?,
          }
        }
      }

      impl #enum_type {
        pub fn from_bytes(b: &[u8]) -> Result<Self, SdkErrorReport> {
          match b {
            #( #byte_variants )*
            other => Err(SdkError::CommonError(
              String::from_utf8_lossy(other).into_owned(),
            ))?,
          }
        }
      }
    }
    .to_string());
}

fn gen_one_sequence_match_arm(
    event_ident: &Ident,
    schema_type_particle: &OpenXmlSchemaTypeParticle,
    child: &OpenXmlSchemaTypeChild,
    gen_context: &GenContext,
    loop_children_ident_set: &mut HashSet<Ident>,
) -> Result<Option<Arm>, BuildErrorReport> {
    let child_type = gen_context
        .type_name_type_map
        .try_get(child.name.as_str())?;

    let child_property_name_ident = child.as_property_name_ident();
    let child_property_type = child_type.r#type(false);

    if !loop_children_ident_set.insert(child_property_name_ident.clone()) {
        return Ok(None);
    }

    match schema_type_particle.as_occurrence() {
        Occurrence::Required | Occurrence::Optional => Ok(Some(parse_quote! {
            _ if #child_property_type::matched_name(#event_ident.name().0) => {
                #child_property_name_ident = Some(std::boxed::Box::new(
                    #child_property_type::deserialize_inner(xml_reader, Some((#event_ident, e_empty)))?,
                ));
            }
        })),
        Occurrence::Repeated => Ok(Some(parse_quote! {
            _ if #child_property_type::matched_name(#event_ident.name().0) => {
                #child_property_name_ident.push(
                    #child_property_type::deserialize_inner(xml_reader, Some((#event_ident, e_empty)))?,
                );
            }
        })),
    }
}

fn gen_child_match_arm(
    event_ident: &Ident,
    child: &OpenXmlSchemaTypeChild,
    child_choice_enum_type: &Type,
    gen_context: &GenContext,
    loop_children_ident_set: &mut HashSet<Ident>,
) -> Result<Option<Arm>, BuildErrorReport> {
    let child_type = gen_context
        .type_name_type_map
        .try_get(child.name.as_str())?;

    let child_variant_name_ident = child.as_last_name_ident();
    let child_variant_type = child_type.r#type(false);

    if !loop_children_ident_set.insert(child_variant_name_ident.clone()) {
        return Ok(None);
    }

    return Ok(Some(parse_quote! {
        _ if #child_variant_type::matched_name(#event_ident.name().0) => {
            children.push(#child_choice_enum_type::#child_variant_name_ident(std::boxed::Box::new(
                #child_variant_type::deserialize_inner(xml_reader, Some((#event_ident, e_empty)))?,
            )))
        }
    }));
}

fn gen_simple_child_match_arm(
    first_name: &str,
    gen_context: &GenContext,
) -> Result<Arm, BuildErrorReport> {
    if let Some(schema_enum) = gen_context.enum_type_enum_map.get(first_name) {
        let simple_type_name = schema_enum.r#type(false);

        return Ok(parse_quote! {
            quick_xml::events::Event::Text(t) => {
                xml_content = Some(#simple_type_name::from_bytes(&t.into_inner())?);
            }
        });
    }

    let simple_type_str = simple_type_mapping(first_name);

    let r#type: Type =
        parse_str(&format!("simple_type::{simple_type_str}")).map_err(BuildError::from)?;

    return Ok(match simple_type_str {
        "Base64BinaryValue" | "DateTimeValue" | "DecimalValue" | "HexBinaryValue"
        | "IntegerValue" | "SByteValue" | "StringValue" => parse_quote! {
            quick_xml::events::Event::Text(t) => {
                xml_content.get_or_insert_with(String::new).push_str(&t.decode().map_err(SdkError::from)?);
            }
        },
        "BooleanValue" | "OnOffValue" | "TrueFalseBlankValue" | "TrueFalseValue" => parse_quote! {
            quick_xml::events::Event::Text(t) => {
                xml_content = Some(parse_bool_bytes(&t.into_inner())?);
            }
        },
        "ByteValue" | "Int16Value" | "Int32Value" | "Int64Value" | "UInt16Value"
        | "UInt32Value" | "UInt64Value" | "DoubleValue" | "SingleValue" => parse_quote! {
            quick_xml::events::Event::Text(t) => {
                xml_content = Some(
                    t.decode().map_err(SdkError::from)?.parse::<#r#type>().map_err(SdkError::from)?
                );
            }
        },
        _ => unreachable!("{simple_type_str}"),
    });
}

fn gen_field_match_arm(
    schema: &OpenXmlSchemaTypeAttribute,
    gen_context: &GenContext,
) -> Result<Arm, BuildErrorReport> {
    let attr_name_ident = schema.as_name_ident();
    let attr_name_str = schema.as_name_str();
    let attr_name_literal: LitByteStr =
        parse_str(&format!("b\"{attr_name_str}\"")).map_err(BuildError::from)?;
    let attr_type = schema.r#type(gen_context)?;

    match schema.r#type.as_ref().unwrap() {
        OpenXmlSchemaTypeAttributeType::ListValue { .. } => {
            return Ok(parse_quote! {
                #attr_name_literal => {
                    #attr_name_ident = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned()
                    );
                }
            });
        }
        OpenXmlSchemaTypeAttributeType::EnumValue { .. } => {
            return Ok(parse_quote! {
              #attr_name_literal => {
                #attr_name_ident = Some(#attr_type::from_bytes(&attr.value)?);
              }
            });
        }
        OpenXmlSchemaTypeAttributeType::SimpleType { r#type } => match r#type.as_str() {
            "Base64BinaryValue" | "DateTimeValue" | "DecimalValue" | "HexBinaryValue"
            | "IntegerValue" | "SByteValue" | "StringValue" => {
                return Ok(parse_quote! {
                  #attr_name_literal => {
                    #attr_name_ident = Some(attr.decode_and_unescape_value(xml_reader.decoder()).map_err(SdkError::from)?.into_owned());
                  }
                });
            }
            "BooleanValue" | "OnOffValue" | "TrueFalseBlankValue" | "TrueFalseValue" => {
                return Ok(parse_quote! {
                  #attr_name_literal => {
                    #attr_name_ident = Some(parse_bool_bytes(&attr.value)?);
                  }
                });
            }
            "ByteValue" | "Int16Value" | "Int32Value" | "Int64Value" | "UInt16Value"
            | "UInt32Value" | "UInt64Value" | "DoubleValue" | "SingleValue" => {
                return Ok(parse_quote! {
                  #attr_name_literal => {
                    #attr_name_ident = Some(
                      attr
                        .decode_and_unescape_value(xml_reader.decoder()).map_err(SdkError::from)?
                        .parse::<#attr_type>().map_err(SdkError::from)?,
                    );
                  }
                });
            }
            _ => unreachable!("{}", r#type),
        },
    }
}
