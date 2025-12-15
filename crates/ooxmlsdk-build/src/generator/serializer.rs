use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{Arm, Ident, ItemFn, ItemImpl, Stmt, Type, parse_str, parse2};

use crate::{
    GenContext,
    models::{OpenXmlSchema, OpenXmlSchemaTypeAttribute, OpenXmlSchemaTypeChild},
    utils::{escape_snake_case, get_or_panic},
};

pub fn gen_serializer(schema: &OpenXmlSchema, gen_context: &GenContext) -> TokenStream {
    let mut token_stream_list: Vec<ItemImpl> = vec![];

    let schema_namespace = gen_context
        .uri_namespace_map
        .get(schema.target_namespace.as_str())
        .ok_or(format!("{:?}", schema.target_namespace))
        .unwrap();

    for schema_enum in &schema.enums {
        let enum_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema.module_name,
            schema_enum.name.to_upper_camel_case()
        ))
        .unwrap();

        let mut variants: Vec<Arm> = vec![];

        for schema_enum_facet in &schema_enum.facets {
            let variant_ident = schema_enum_facet.as_variant_ident();
            let variant_value = &schema_enum_facet.value;

            variants.push(
                parse2(quote! {
                  Self::#variant_ident => #variant_value.to_string(),
                })
                .unwrap(),
            );
        }

        token_stream_list.push(
            parse2(quote! {
              impl #enum_type {
                pub fn to_xml(&self) -> String {
                  match self {
                    #( #variants )*
                  }
                }
              }
            })
            .unwrap(),
        );

        token_stream_list.push(
            parse2(quote! {
              impl std::fmt::Display for #enum_type {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                  write!(f, "{}", self.to_xml())
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

        let struct_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema.module_name,
            schema_type.class_name.to_upper_camel_case()
        ))
        .unwrap();

        let child_choice_enum_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}ChildChoice",
            &schema.module_name,
            schema_type.class_name.to_upper_camel_case()
        ))
        .unwrap();

        let (_, last_name) = schema_type.split_name();
        let (last_name_prefix, last_name_suffix) = last_name.split_once(':').unwrap();

        let last_name_start_tag = format!("<{last_name}");
        let last_name_suffix_start_tag = format!("<{last_name_suffix}");

        let last_name_end_tag = format!("</{last_name}>");
        let last_name_suffix_end_tag = format!("</{last_name_suffix}>");

        let end_tag_writer;

        let end_writer;

        let mut variants: Vec<TokenStream> = vec![];

        let mut children_writer = quote! {};

        let mut child_arms: Vec<Arm> = vec![];

        for attr in &schema_type.attributes {
            variants.push(gen_attr(attr));
        }

        if schema_type.base_class == "OpenXmlLeafTextElement" {
            children_writer = quote! {
              if let Some(xml_content) = &self.xml_content {
                writer.write_str(&quick_xml::escape::escape(xml_content.to_string()))?;
              }
            };

            end_tag_writer = quote! {
              writer.write_char('>')?;
            };

            end_writer = quote! {
              if xmlns_prefix == #last_name_prefix {
                writer.write_str(#last_name_suffix_end_tag)?;
              } else {
                writer.write_str(#last_name_end_tag)?;
              }
            };
        } else if schema_type.base_class == "OpenXmlLeafElement" {
            children_writer = quote! {};

            end_tag_writer = quote! {};

            end_writer = quote! {
              writer.write_str("/>")?;
            };
        } else if schema_type.base_class == "OpenXmlCompositeElement"
            || schema_type.base_class == "CustomXmlElement"
            || schema_type.base_class == "OpenXmlPartRootElement"
            || schema_type.base_class == "SdtElement"
        {
            if schema_type.children.is_empty() {
                end_tag_writer = quote! {};

                end_writer = quote! {
                  writer.write_str("/>")?;
                };
            } else if schema_type.is_one_sequence_flatten() {
                let mut child_map: HashMap<&str, &OpenXmlSchemaTypeChild> =
                    HashMap::with_capacity(schema_type.children.len());
                for child in &schema_type.children {
                    child_map.insert(&child.name, child);
                }

                let mut child_stmt_list: Vec<Stmt> = vec![];

                for p in &schema_type.particle.items {
                    let child = child_map.get(p.name.as_str()).ok_or(&p.name).unwrap();

                    let child_name_ident_raw = if child.property_name.is_empty() {
                        child.name.rsplit('/').next().ok_or(&child.name).unwrap()
                    } else {
                        &child.property_name
                    };

                    let child_name_ident: Ident =
                        parse_str(&escape_snake_case(child_name_ident_raw)).unwrap();

                    if p.occurs.is_empty() {
                        child_stmt_list.push(
                            parse2(quote! {
                              self.#child_name_ident.write_xml(writer, xmlns_prefix)?;
                            })
                            .unwrap(),
                        );
                    } else if p.occurs[0].min == 0 && p.occurs[0].max == 1 {
                        child_stmt_list.push(
                            parse2(quote! {
                              if let Some(#child_name_ident) = &self.#child_name_ident {
                                #child_name_ident.write_xml(writer, xmlns_prefix)?;
                              }
                            })
                            .unwrap(),
                        );
                    } else {
                        child_stmt_list.push(
                            parse2(quote! {
                              for child in &self.#child_name_ident {
                                child.write_xml(writer, xmlns_prefix)?;
                              }
                            })
                            .unwrap(),
                        );
                    }
                }

                children_writer = quote! {
                  #( #child_stmt_list )*
                };

                end_tag_writer = quote! {
                  writer.write_char('>')?;
                };

                end_writer = quote! {
                  if xmlns_prefix == #last_name_prefix {
                    writer.write_str(#last_name_suffix_end_tag)?;
                  } else {
                    writer.write_str(#last_name_end_tag)?;
                  }
                };
            } else {
                for child in &schema_type.children {
                    child_arms.push(gen_child_arm(child, &child_choice_enum_type));
                }

                children_writer = quote! {
                  for child in &self.children {
                    match child {
                      #( #child_arms )*
                    };
                  }
                };

                end_tag_writer = quote! {
                  writer.write_char('>')?;
                };

                end_writer = quote! {
                  if xmlns_prefix == #last_name_prefix {
                    writer.write_str(#last_name_suffix_end_tag)?;
                  } else {
                    writer.write_str(#last_name_end_tag)?;
                  }
                };
            }
        } else if schema_type.is_derived {
            let base_class_type = get_or_panic!(
                gen_context.type_name_type_map,
                &schema_type.name[0..schema_type.name.find('/').unwrap() + 1]
            );

            for attr in &base_class_type.attributes {
                variants.push(gen_attr(attr));
            }

            let mut children_map: HashMap<&str, OpenXmlSchemaTypeChild> = HashMap::new();

            for c in &schema_type.children {
                children_map.insert(&c.name, c.clone());
            }

            for c in &base_class_type.children {
                children_map.insert(&c.name, c.clone());
            }

            let children: Vec<OpenXmlSchemaTypeChild> = children_map.into_values().collect();

            for child in &children {
                child_arms.push(gen_child_arm(child, &child_choice_enum_type));
            }

            if children.is_empty() {
                if base_class_type.base_class == "OpenXmlLeafTextElement" {
                    children_writer = quote! {
                      if let Some(xml_content) = &self.xml_content {
                        writer.write_str(&quick_xml::escape::escape(xml_content.to_string()))?;
                      }
                    };

                    end_tag_writer = quote! {
                      writer.write_char('>')?;
                    };

                    end_writer = quote! {
                      if xmlns_prefix == #last_name_prefix {
                        writer.write_str(#last_name_suffix_end_tag)?;
                      } else {
                        writer.write_str(#last_name_end_tag)?;
                      }
                    };
                } else {
                    end_tag_writer = quote! {};

                    end_writer = quote! {
                      writer.write_str("/>")?;
                    };
                }
            } else if schema_type.is_one_sequence_flatten()
                && base_class_type.composite_type == "OneSequence"
            {
                let mut child_map: HashMap<&str, &OpenXmlSchemaTypeChild> = HashMap::new();

                for child in &schema_type.children {
                    child_map.insert(&child.name, child);
                }

                let mut child_stmt_list: Vec<Stmt> = vec![];

                for p in &schema_type.particle.items {
                    let child = child_map.get(p.name.as_str()).ok_or(&p.name).unwrap();

                    let child_name_ident_raw = if child.property_name.is_empty() {
                        child.name.rsplit('/').next().ok_or(&child.name).unwrap()
                    } else {
                        &child.property_name
                    };

                    let child_name_ident: Ident =
                        parse_str(&escape_snake_case(child_name_ident_raw)).unwrap();

                    if p.occurs.is_empty() {
                        child_stmt_list.push(
                            parse2(quote! {
                              self.#child_name_ident.write_xml(writer, xmlns_prefix)?;
                            })
                            .unwrap(),
                        );
                    } else if p.occurs[0].min == 0 && p.occurs[0].max == 1 {
                        child_stmt_list.push(
                            parse2(quote! {
                              if let Some(#child_name_ident) = &self.#child_name_ident {
                                #child_name_ident.write_xml(writer, xmlns_prefix)?;
                              }
                            })
                            .unwrap(),
                        );
                    } else {
                        child_stmt_list.push(
                            parse2(quote! {
                              for child in &self.#child_name_ident {
                                child.write_xml(writer, xmlns_prefix)?;
                              }
                            })
                            .unwrap(),
                        );
                    }
                }

                children_writer = quote! {
                  #( #child_stmt_list )*
                };

                end_tag_writer = quote! {
                  writer.write_char('>')?;
                };

                end_writer = quote! {
                  if xmlns_prefix == #last_name_prefix {
                    writer.write_str(#last_name_suffix_end_tag)?;
                  } else {
                    writer.write_str(#last_name_end_tag)?;
                  }
                };
            } else {
                children_writer = quote! {
                  for child in &self.children {
                    match child {
                      #( #child_arms )*
                    };
                  }
                };

                end_tag_writer = quote! {
                  writer.write_char('>')?;
                };

                end_writer = quote! {
                  if xmlns_prefix == #last_name_prefix {
                    writer.write_str(#last_name_suffix_end_tag)?;
                  } else {
                    writer.write_str(#last_name_end_tag)?;
                  }
                };
            }
        } else {
            panic!("{schema_type:?}");
        };

        let attr_writer = quote! {
          #( #variants )*
        };

        let mut xmlns_attr_writer_list: Vec<Stmt> = vec![];

        let mut xml_header_writer: Option<Stmt> = None;

        if !schema_type.part.is_empty() || schema_type.base_class == "OpenXmlPartRootElement" {
            xml_header_writer = Some(
                parse2(quote! {
                    writer.write_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n")?;
                })
                .unwrap(),
            );
        }

        if !schema_type.part.is_empty()
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
            xmlns_attr_writer_list.push(
                parse2(quote! {
                  if let Some(xmlns) = &self.xmlns {
                    writer.write_str(r#" xmlns=""#)?;
                    writer.write_str(xmlns)?;
                    writer.write_str("\"")?;
                  }
                })
                .unwrap(),
            );

            xmlns_attr_writer_list.push(
                parse2(quote! {
                  for (k, v) in &self.xmlns_map {
                    writer.write_str(" xmlns:")?;
                    writer.write_str(k)?;
                    writer.write_str("=\"")?;
                    writer.write_str(v)?;
                    writer.write_str("\"")?;
                  }
                })
                .unwrap(),
            );

            xmlns_attr_writer_list.push(
                parse2(quote! {
                  if let Some(mc_ignorable) = &self.mc_ignorable {
                    writer.write_str(r#" mc:Ignorable=""#)?;
                    writer.write_str(mc_ignorable)?;
                    writer.write_str("\"")?;
                  }
                })
                .unwrap(),
            );
        }

        let xmlns_uri_str = &schema_namespace.uri;

        let xmlns_prefix_str = &schema_namespace.prefix;

        let to_xml_fn: ItemFn = if !schema_type.part.is_empty()
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
            parse2(quote! {
              pub fn to_xml(&self) -> Result<String, std::fmt::Error> {
                let mut writer = String::with_capacity(32);

                self.write_xml(
                  &mut writer,
                  if let Some(xmlns) = &self.xmlns {
                    if xmlns == #xmlns_uri_str {
                      #xmlns_prefix_str
                    } else {
                      ""
                    }
                  } else {
                    ""
                  },
                )?;

                Ok(writer)
              }
            })
            .unwrap()
        } else {
            parse2(quote! {
              pub fn to_xml(&self) -> Result<String, std::fmt::Error> {
                let mut writer = String::with_capacity(32);

                self.write_xml(&mut writer, "")?;

                Ok(writer)
              }
            })
            .unwrap()
        };

        token_stream_list.push(
            parse2(quote! {
              impl #struct_type {
                #to_xml_fn

                pub(crate) fn write_xml<W: std::fmt::Write>(
                  &self,
                  writer: &mut W,
                  xmlns_prefix: &str,
                ) -> Result<(), std::fmt::Error> {
                  #xml_header_writer

                  if xmlns_prefix == #last_name_prefix {
                    writer.write_str(#last_name_suffix_start_tag)?;
                  } else {
                    writer.write_str(#last_name_start_tag)?;
                  }

                  #( #xmlns_attr_writer_list )*

                  #attr_writer

                  #end_tag_writer

                  #children_writer

                  #end_writer

                  Ok(())
                }
              }
            })
            .unwrap(),
        );

        token_stream_list.push(
            parse2(quote! {
              impl std::fmt::Display for #struct_type {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                  write!(f, "{}", self.to_xml()?)
                }
              }
            })
            .unwrap(),
        );
    }

    quote! {
      #( #token_stream_list )*
    }
}

fn gen_attr(schema: &OpenXmlSchemaTypeAttribute) -> TokenStream {
    let attr_value_ident = schema.as_name_ident();
    let attr_name_str = schema.as_name_str();

    let attr_name_str_fmt = format!(" {attr_name_str}=\"");

    if schema.is_validator_required() {
        quote! {
          writer.write_str(#attr_name_str_fmt)?;
          writer.write_str(&quick_xml::escape::escape(self.#attr_value_ident))?;
          writer.write_char('"')?;
        }
    } else {
        quote! {
          if let Some(#attr_value_ident) = &self.#attr_value_ident {
            writer.write_str(#attr_name_str_fmt)?;
            writer.write_str(&quick_xml::escape::escape(#attr_value_ident))?;
            writer.write_char('"')?;
          }
        }
    }
}

fn gen_child_arm(child: &OpenXmlSchemaTypeChild, child_choice_enum_type: &Type) -> Arm {
    let child_name_ident = child.as_last_name_ident();

    parse2(quote! {
      #child_choice_enum_type::#child_name_ident(child) => child.write_xml(writer, xmlns_prefix)?,
    })
    .unwrap()
}
