use proc_macro2::Literal;
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

    let deserialize_attributes_fn =
        gen_deserialize_attributes_fn(schema, schema_type, gen_context)?;

    let deserialize_children_fn = gen_deserialize_children_fn(schema, schema_type, gen_context)?;

    return Ok(quote! {
        impl Deserializeable for #struct_type {
            #deserialize_attributes_fn

            #deserialize_children_fn
        }
    }
    .to_string());
}

fn gen_deserialize_attributes_fn(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    gen_context: &GenContext,
) -> Result<Option<ItemFn>, BuildErrorReport> {
    let schema_type_class_name = schema_type.class_name_ident().to_string();

    let xml_reader_ident = format_ident!("_xml_reader");
    let xml_event_ident = format_ident!("_xml_event");
    let attr_ident = format_ident!("attr");

    let mut declarations = vec![];
    let mut matchers = vec![];
    let mut reassignments = vec![];

    if schema_type.base_class == "OpenXmlLeafTextElement"
        || schema_type.base_class == "OpenXmlLeafElement"
        || schema_type.base_class == "OpenXmlCompositeElement"
        || schema_type.base_class == "CustomXmlElement"
        || schema_type.base_class == "OpenXmlPartRootElement"
        || schema_type.base_class == "SdtElement"
    {
        for schema_type_attribute in &schema_type.attributes {
            let t = TypeDeserializer::from_open_xml_schema_type_attribute(
                schema_type_attribute,
                &xml_reader_ident,
                &attr_ident,
                gen_context,
            )?;

            declarations.push(t.declaration);
            matchers.extend(t.matchers);
            reassignments.push(t.reassignment);
        }
    } else if schema_type.is_derived {
        let (type_base_class, _) = schema_type.split_name();

        let base_class_type = gen_context
            .type_name_type_map
            .try_get(format!("{type_base_class}/").as_str())?;

        for schema_type_attribute in base_class_type
            .attributes
            .iter()
            .chain(schema_type.attributes.iter())
        {
            let t = TypeDeserializer::from_open_xml_schema_type_attribute(
                schema_type_attribute,
                &xml_reader_ident,
                &attr_ident,
                gen_context,
            )?;

            declarations.push(t.declaration);
            matchers.extend(t.matchers);
            reassignments.push(t.reassignment);
        }
    };

    if schema.needs_xmlns(schema_type) {
        let t = TypeDeserializer::xmlns(&xml_reader_ident, &attr_ident);

        declarations.push(t.declaration);
        matchers.extend(t.matchers);
        reassignments.push(t.reassignment);
    }

    if declarations.is_empty() {
        return Ok(None);
    }

    Ok(Some(parse_quote!(
        fn deserialize_attributes<'de>(
            mut self,
            #xml_reader_ident: &impl XmlReader<'de>,
            #xml_event_ident: quick_xml::events::BytesStart<'de>,
        ) -> Result<Self, SdkErrorReport> {
            #( #declarations )*

            for #attr_ident in #xml_event_ident.attributes().with_checks(false) {
                let #attr_ident = #attr_ident.map_err(SdkError::from)?;

                #[allow(clippy::single_match)]
                match #attr_ident.key.0 {
                    #( #matchers )*
                    other => {
                        tracing::warn!(
                            "Unhandled attribute: ({}) from schema: ({})",
                            String::from_utf8_lossy(other),
                            #schema_type_class_name
                        );
                    }
                }
            }

            #( #reassignments )*

            Ok(self)
        }
    )))
}

fn gen_deserialize_children_fn(
    schema: &OpenXmlSchema,
    schema_type: &OpenXmlSchemaType,
    gen_context: &GenContext,
) -> Result<Option<ItemFn>, BuildErrorReport> {
    let schema_type_class_name = schema_type.class_name_ident().to_string();
    let mut children_ident_set: HashSet<Ident> = HashSet::new();

    let xml_reader_ident = format_ident!("xml_reader");

    let mut declarations = vec![];
    let mut matchers = vec![];
    let mut reassignments = vec![];

    TypeDeserializer::from_schema_type(
        schema_type,
        schema,
        &xml_reader_ident,
        &mut children_ident_set,
        gen_context,
    )?
    .into_iter()
    .for_each(|t| {
        declarations.push(t.declaration);
        matchers.extend(t.matchers);
        reassignments.push(t.reassignment);
    });

    if declarations.is_empty() {
        return Ok(None);
    }

    return Ok(Some(parse_quote! {
        fn deserialize_children<'de>(
            mut self,
            #xml_reader_ident: &mut impl XmlReader<'de>,
        ) -> Result<Self, SdkErrorReport> {
            #( #declarations )*

            loop {
                match BytesEvent::expect(#xml_reader_ident)? {
                    #( #matchers )*
                    BytesEvent::End(bytes_end) if Self::matched_bytes_end(&bytes_end) => {
                        break;
                    }
                    other => {
                        tracing::warn!(
                            "Unhandled event: ({other:?}) from schema: ({})",
                            #schema_type_class_name
                        );
                    }
                }
            }

            #( #reassignments )*

            Ok(self)
        }
    }));
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
                Self::try_from(s.as_bytes())
            }
        }

        impl TryFrom<&[u8]> for #enum_type {
            type Error = SdkErrorReport;

            fn try_from(b: &[u8]) -> Result<Self, <Self as TryFrom<&[u8]>>::Error> {
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
    schema_type_child: &OpenXmlSchemaTypeChild,
    schema_type_particle: &OpenXmlSchemaTypeParticle,
    xml_reader_ident: &Ident,
    loop_children_ident_set: &mut HashSet<Ident>,
    gen_context: &GenContext,
) -> Result<Option<Arm>, BuildErrorReport> {
    let child_property_schema_type = gen_context
        .type_name_type_map
        .try_get(schema_type_child.name.as_str())?;
    let child_property_name_ident = schema_type_child.as_property_name_ident();
    let child_property_type = child_property_schema_type.r#type(false);

    if !loop_children_ident_set.insert(child_property_name_ident.clone()) {
        return Ok(None);
    }

    match schema_type_particle.as_occurrence() {
        Occurrence::Required | Occurrence::Optional => Ok(Some(parse_quote! {
            BytesEvent::BytesStart(bytes_start, is_empty) if #child_property_type::matched_bytes_start(&bytes_start) => {
                let mut child = #child_property_type::default().deserialize_attributes(#xml_reader_ident, bytes_start)?;

                if !is_empty {
                    child = child.deserialize_children(#xml_reader_ident)?;
                }

                #child_property_name_ident = Some(std::boxed::Box::new(child));
            }
        })),
        Occurrence::Repeated => Ok(Some(parse_quote! {
            BytesEvent::BytesStart(bytes_start, is_empty) if #child_property_type::matched_bytes_start(&bytes_start) => {
                let mut child = #child_property_type::default().deserialize_attributes(#xml_reader_ident, bytes_start)?;

                if !is_empty {
                    child = child.deserialize_children(#xml_reader_ident)?;
                }

                #child_property_name_ident.push(child);
            }
        })),
    }
}

fn gen_child_match_arm(
    schema_type_child: &OpenXmlSchemaTypeChild,
    schema_child_choice_type: &Type,
    xml_reader_ident: &Ident,
    loop_children_ident_set: &mut HashSet<Ident>,
    gen_context: &GenContext,
) -> Result<Option<Arm>, BuildErrorReport> {
    let child_variant_schema_type = gen_context
        .type_name_type_map
        .try_get(schema_type_child.name.as_str())?;
    let child_variant_name_ident = schema_type_child.as_last_name_ident();
    let child_variant_type = child_variant_schema_type.r#type(false);

    if !loop_children_ident_set.insert(child_variant_name_ident.clone()) {
        return Ok(None);
    }

    return Ok(Some(parse_quote! {
        BytesEvent::BytesStart(bytes_start, is_empty) if #child_variant_type::matched_bytes_start(&bytes_start) => {
            let mut child = #child_variant_type::default().deserialize_attributes(#xml_reader_ident, bytes_start)?;

            if !is_empty {
                child = child.deserialize_children(#xml_reader_ident)?;
            }

            children.push(#schema_child_choice_type::#child_variant_name_ident(std::boxed::Box::new(child)))
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
            BytesEvent::BytesText(bytes_text) => {
                xml_content = Some(#simple_type_name::try_from(bytes_text.into_inner().as_ref())?);
            }
        });
    }

    let simple_type_str = simple_type_mapping(first_name);

    let r#type: Type =
        parse_str(&format!("simple_type::{simple_type_str}")).map_err(BuildError::from)?;

    return Ok(match simple_type_str {
        "Base64BinaryValue" | "DateTimeValue" | "DecimalValue" | "HexBinaryValue"
        | "IntegerValue" | "SByteValue" | "StringValue" => parse_quote! {
            BytesEvent::BytesText(bytes_text) => {
                xml_content.get_or_insert_with(String::new).push_str(&bytes_text.decode().map_err(SdkError::from)?);
            }
        },
        "BooleanValue" | "OnOffValue" | "TrueFalseBlankValue" | "TrueFalseValue" => parse_quote! {
            BytesEvent::BytesText(bytes_text) => {
                xml_content = Some(parse_bool_bytes(&bytes_text.into_inner())?);
            }
        },
        "ByteValue" | "Int16Value" | "Int32Value" | "Int64Value" | "UInt16Value"
        | "UInt32Value" | "UInt64Value" | "DoubleValue" | "SingleValue" => parse_quote! {
            BytesEvent::BytesText(bytes_text) => {
                xml_content = Some(
                    bytes_text.decode().map_err(SdkError::from)?.parse::<#r#type>().map_err(SdkError::from)?
                );
            }
        },
        _ => unreachable!("{simple_type_str}"),
    });
}

struct TypeDeserializer {
    declaration: Stmt,
    matchers: Vec<Arm>,
    reassignment: Stmt,
}

impl TypeDeserializer {
    fn xmlns(xml_reader_ident: &Ident, attr_ident: &Ident) -> Self {
        let name_ident = format_ident!("xmlns");

        let declaration = parse_quote! {
            let mut #name_ident = XmlNamespace::default();
        };

        let matchers = vec![parse_quote! {
            _ if #name_ident.deserialize_attributes(#xml_reader_ident, &#attr_ident)?.is_some() => {
                continue;
            }
        }];

        let reassignment = parse_quote! {
            self.#name_ident = #name_ident;
        };

        Self {
            declaration,
            matchers,
            reassignment,
        }
    }

    fn xml_content(type_base_class: &str, gen_context: &GenContext) -> Self {
        let name_ident = format_ident!("xml_content");

        let declaration = parse_quote! {
            let mut xml_content = None;
        };

        let matchers =
            vec![gen_simple_child_match_arm(type_base_class, gen_context).expect("xml_content")];

        let reassignment = parse_quote! {
            self.#name_ident = #name_ident;
        };

        Self {
            declaration,
            matchers,
            reassignment,
        }
    }

    fn from_open_xml_schema_type_attribute(
        schema_type_attribute: &OpenXmlSchemaTypeAttribute,
        xml_reader_ident: &Ident,
        attr_ident: &Ident,
        gen_context: &GenContext,
    ) -> Result<Self, BuildErrorReport> {
        let attr_name_ident = schema_type_attribute.as_name_ident();
        let attr_name_str = schema_type_attribute.as_name_str();
        let attr_name_literal = Literal::byte_string(attr_name_str.as_bytes());
        let attr_type = schema_type_attribute.r#type(gen_context)?;

        let declaration = parse_quote! {
            let mut #attr_name_ident = None;
        };

        let matchers = match schema_type_attribute.r#type.as_ref().unwrap() {
            OpenXmlSchemaTypeAttributeType::ListValue { .. } => parse_quote! {
                #attr_name_literal => {
                    #attr_name_ident = Some(
                        #attr_ident.decode_and_unescape_value(#xml_reader_ident.decoder())
                            .map_err(SdkError::from)?
                            .into_owned()
                    );
                }
            },
            OpenXmlSchemaTypeAttributeType::EnumValue { .. } => parse_quote! {
              #attr_name_literal => {
                #attr_name_ident = Some(#attr_type::try_from(#attr_ident.value.as_ref())?);
              }
            },
            OpenXmlSchemaTypeAttributeType::SimpleType { r#type } => match r#type.as_str() {
                "Base64BinaryValue" | "DateTimeValue" | "DecimalValue" | "HexBinaryValue"
                | "IntegerValue" | "SByteValue" | "StringValue" => parse_quote! {
                  #attr_name_literal => {
                    #attr_name_ident = Some(#attr_ident.decode_and_unescape_value(#xml_reader_ident.decoder())
                        .map_err(SdkError::from)?.into_owned());
                  }
                },
                "BooleanValue" | "OnOffValue" | "TrueFalseBlankValue" | "TrueFalseValue" => {
                    parse_quote! {
                      #attr_name_literal => {
                        #attr_name_ident = Some(parse_bool_bytes(&#attr_ident.value)?);
                      }
                    }
                }
                "ByteValue" | "Int16Value" | "Int32Value" | "Int64Value" | "UInt16Value"
                | "UInt32Value" | "UInt64Value" | "DoubleValue" | "SingleValue" => {
                    parse_quote! {
                      #attr_name_literal => {
                        #attr_name_ident = Some(
                          attr
                            .decode_and_unescape_value(#xml_reader_ident.decoder()).map_err(SdkError::from)?
                            .parse::<#attr_type>().map_err(SdkError::from)?,
                        );
                      }
                    }
                }
                _ => unreachable!("{}", r#type),
            },
        };

        let reassignment = if schema_type_attribute.is_validator_required() {
            parse_quote! {
                self.#attr_name_ident = #attr_name_ident
                    .ok_or_else(|| SdkError::CommonError(#attr_name_str.to_string()))?;
            }
        } else {
            parse_quote! {
                self.#attr_name_ident = #attr_name_ident;
            }
        };

        Ok(Self {
            declaration,
            matchers,
            reassignment,
        })
    }

    fn from_open_xml_schema_type_particle(
        schema_type_particle: &OpenXmlSchemaTypeParticle,
        schema_type: &OpenXmlSchemaType,
        xml_reader_ident: &Ident,
        children_ident_set: &mut HashSet<Ident>,
        gen_context: &GenContext,
    ) -> Result<Option<Self>, BuildErrorReport> {
        let child_map = schema_type.child_map();
        let schema_type_child = child_map.try_get(schema_type_particle.name.as_str())?;

        let child_property_name_str = schema_type_child.as_property_name_str();
        let child_property_name_ident = schema_type_child.as_property_name_ident();

        if let Some(arm) = gen_one_sequence_match_arm(
            schema_type_child,
            schema_type_particle,
            xml_reader_ident,
            children_ident_set,
            gen_context,
        )? {
            let matchers = vec![arm];

            return match schema_type_particle.as_occurrence() {
                Occurrence::Required => Ok(Some(Self {
                    declaration: parse_quote! {
                        let mut #child_property_name_ident = None;
                    },
                    matchers,
                    reassignment: parse_quote! {
                        self.#child_property_name_ident = #child_property_name_ident
                            .ok_or_else(|| SdkError::CommonError(#child_property_name_str.to_string()))?;
                    },
                })),
                Occurrence::Optional => Ok(Some(Self {
                    declaration: parse_quote! {
                        let mut #child_property_name_ident = None;
                    },
                    matchers,
                    reassignment: parse_quote! {
                        self.#child_property_name_ident = #child_property_name_ident;
                    },
                })),
                Occurrence::Repeated => Ok(Some(Self {
                    declaration: parse_quote! {
                        let mut #child_property_name_ident = vec![];
                    },
                    matchers,
                    reassignment: parse_quote! {
                        self.#child_property_name_ident = #child_property_name_ident;
                    },
                })),
            };
        }

        return Ok(None);
    }

    fn from_open_xml_schema_type_children(
        schema_type_children: &[OpenXmlSchemaTypeChild],
        schema_child_choice_type: &Type,
        xml_reader_ident: &Ident,
        children_ident_set: &mut HashSet<Ident>,
        gen_context: &GenContext,
    ) -> Result<Option<Self>, BuildErrorReport> {
        let mut type_deserializer = TypeDeserializer {
            declaration: parse_quote! {
                let mut children = vec![];
            },
            matchers: vec![],
            reassignment: parse_quote! {
                self.children = children;
            },
        };

        for schema_type_child in schema_type_children {
            if let Some(matcher) = gen_child_match_arm(
                schema_type_child,
                schema_child_choice_type,
                xml_reader_ident,
                children_ident_set,
                gen_context,
            )? {
                type_deserializer.matchers.push(matcher);
            }
        }

        if type_deserializer.matchers.is_empty() {
            return Ok(None);
        }

        return Ok(Some(type_deserializer));
    }

    fn from_schema_type(
        schema_type: &OpenXmlSchemaType,
        schema: &OpenXmlSchema,
        xml_reader_ident: &Ident,
        children_ident_set: &mut HashSet<Ident>,
        gen_context: &GenContext,
    ) -> Result<Vec<Self>, BuildErrorReport> {
        let (type_base_class, _) = schema_type.split_name();

        match schema_type.base_class.as_str() {
            "OpenXmlLeafElement" => return Ok(vec![]),
            "OpenXmlLeafTextElement" => {
                return Ok(vec![TypeDeserializer::xml_content(
                    type_base_class,
                    gen_context,
                )]);
            }
            "OpenXmlCompositeElement"
            | "CustomXmlElement"
            | "OpenXmlPartRootElement"
            | "SdtElement" => {
                if schema_type.is_one_sequence_flatten() {
                    let mut type_deserializers = vec![];

                    for schema_type_particle in &schema_type.particle.items {
                        if let Some(type_deserializer) =
                            TypeDeserializer::from_open_xml_schema_type_particle(
                                schema_type_particle,
                                schema_type,
                                xml_reader_ident,
                                children_ident_set,
                                gen_context,
                            )?
                        {
                            type_deserializers.push(type_deserializer);
                        }
                    }

                    return Ok(type_deserializers);
                } else {
                    let mut type_deserializers = vec![];

                    if !schema_type.children.is_empty() {
                        let schema_child_choice_type = schema.enum_child_choice_type(schema_type);

                        if let Some(type_deserializer) =
                            TypeDeserializer::from_open_xml_schema_type_children(
                                &schema_type.children,
                                &schema_child_choice_type,
                                xml_reader_ident,
                                children_ident_set,
                                gen_context,
                            )?
                        {
                            type_deserializers.push(type_deserializer);
                        }
                    }

                    return Ok(type_deserializers);
                }
            }
            _ if schema_type.is_derived => {
                let base_class_type = gen_context
                    .type_name_type_map
                    .try_get(format!("{type_base_class}/").as_str())?;

                let mut type_deserializers = vec![];

                if schema_type.is_one_sequence_flatten()
                    && base_class_type.composite_type == Some(CompositeType::OneSequence)
                {
                    for schema_type_particle in &schema_type.particle.items {
                        if let Some(type_deserializer) =
                            TypeDeserializer::from_open_xml_schema_type_particle(
                                schema_type_particle,
                                schema_type,
                                xml_reader_ident,
                                children_ident_set,
                                gen_context,
                            )?
                        {
                            type_deserializers.push(type_deserializer);
                        };
                    }
                } else if !schema_type.children.is_empty() {
                    let schema_child_choice_type = schema.enum_child_choice_type(schema_type);

                    if let Some(type_deserializer) =
                        TypeDeserializer::from_open_xml_schema_type_children(
                            &schema_type.children,
                            &schema_child_choice_type,
                            xml_reader_ident,
                            children_ident_set,
                            gen_context,
                        )?
                    {
                        type_deserializers.push(type_deserializer);
                    }
                } else if base_class_type.base_class == "OpenXmlLeafTextElement" {
                    type_deserializers
                        .push(TypeDeserializer::xml_content(type_base_class, gen_context));
                }

                return Ok(type_deserializers);
            }
            _ => {
                panic!("Unhandled schema_type: {schema_type:?}");
            }
        }
    }
}
