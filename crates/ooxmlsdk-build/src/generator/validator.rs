use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{Arm, Ident, ItemImpl, Stmt, Type, parse_str, parse2};

use crate::{
    GenContext,
    models::{
        Occurrence, OpenXmlNamespace, OpenXmlSchema, OpenXmlSchemaTypeAttribute,
        OpenXmlSchemaTypeChild,
    },
    utils::{escape_snake_case, get_or_panic},
};

pub fn gen_validators(schema: &OpenXmlSchema, gen_context: &GenContext) -> TokenStream {
    let mut token_stream_list: Vec<ItemImpl> = vec![];

    let schema_namespace = get_or_panic!(
        gen_context.uri_namespace_map,
        schema.target_namespace.as_str()
    );

    for t in &schema.types {
        if t.is_abstract {
            continue;
        }

        let struct_type: Type = parse_str(&format!(
            "crate::schemas::{}::{}",
            &schema.module_name,
            t.class_name.to_upper_camel_case()
        ))
        .unwrap();

        let mut attr_validator_stmt_list: Vec<Stmt> = vec![];

        let mut children_validator_stmt_list: Vec<Stmt> = vec![];

        for attr in &t.attributes {
            attr_validator_stmt_list.extend(gen_attr_validator_stmt_list(
                attr,
                schema_namespace,
                gen_context,
            ));
        }

        if t.base_class == "OpenXmlLeafTextElement" || t.base_class == "OpenXmlLeafElement" {
        } else if t.base_class == "OpenXmlCompositeElement"
            || t.base_class == "CustomXmlElement"
            || t.base_class == "OpenXmlPartRootElement"
            || t.base_class == "SdtElement"
        {
            if t.is_one_sequence_flatten() {
                let mut child_map: HashMap<&str, &OpenXmlSchemaTypeChild> = HashMap::new();

                for child in &t.children {
                    child_map.insert(&child.name, child);
                }

                for schema_type_particle in &t.particle.items {
                    let child = child_map
                        .get(schema_type_particle.name.as_str())
                        .ok_or(&schema_type_particle.name)
                        .unwrap();
                    let child_name_ident = child.as_property_name_ident();

                    match schema_type_particle.as_occurrence() {
                        Occurrence::Required => {
                            children_validator_stmt_list.push(
                                parse2(quote! {
                                  if !self.#child_name_ident.validate()? {
                                      return Ok(false);
                                  }
                                })
                                .unwrap(),
                            );
                        }
                        Occurrence::Optional => {
                            children_validator_stmt_list.push(
                                parse2(quote! {
                                    if let Some(#child_name_ident) = &self.#child_name_ident && !#child_name_ident.validate()? {
                                        return Ok(false);
                                    }
                                })
                                .unwrap(),
                        );
                        }
                        Occurrence::Repeated => {
                            children_validator_stmt_list.push(
                                parse2(quote! {
                                    for child in &self.#child_name_ident {
                                        if !child.validate()? {
                                            return Ok(false);
                                        }
                                    }
                                })
                                .unwrap(),
                            );
                        }
                    }
                }
            } else {
                let child_choice_enum_type: Type = parse_str(&format!(
                    "crate::schemas::{}::{}ChildChoice",
                    &schema.module_name,
                    t.class_name.to_upper_camel_case()
                ))
                .unwrap();

                let mut child_match_arm_list: Vec<Arm> = vec![];

                for child in &t.children {
                    let child_name_list: Vec<&str> = child.name.split('/').collect();

                    let child_rename_ser_str = child_name_list
                        .last()
                        .ok_or(format!("{:?}", child.name))
                        .unwrap();

                    let child_variant_name_ident: Ident =
                        parse_str(&child_rename_ser_str.to_upper_camel_case()).unwrap();

                    child_match_arm_list.push(
            parse2(quote! {
              #child_choice_enum_type::#child_variant_name_ident(c) => if !c.validate()? {
                return Ok(false);
              },
            })
            .unwrap(),
          );
                }

                if !t.children.is_empty() {
                    children_validator_stmt_list.push(
                        parse2(quote! {
                          for child in &self.children {
                            match child {
                              #( #child_match_arm_list )*
                            }
                          }
                        })
                        .unwrap(),
                    );
                }
            }
        } else if t.is_derived {
            let base_class_type = get_or_panic!(
                gen_context.type_name_type_map,
                &t.name[0..t.name.find('/').unwrap() + 1]
            );

            for attr in &base_class_type.attributes {
                attr_validator_stmt_list.extend(gen_attr_validator_stmt_list(
                    attr,
                    schema_namespace,
                    gen_context,
                ));
            }

            if t.is_one_sequence_flatten() && base_class_type.composite_type == "OneSequence" {
                let mut child_map: HashMap<&str, &OpenXmlSchemaTypeChild> = HashMap::new();

                for child in &t.children {
                    child_map.insert(&child.name, child);
                }

                for schema_type_particle in &t.particle.items {
                    let child = child_map
                        .get(schema_type_particle.name.as_str())
                        .ok_or(&schema_type_particle.name)
                        .unwrap();
                    let child_name_ident = child.as_property_name_ident();

                    match schema_type_particle.as_occurrence() {
                        Occurrence::Required => {
                            children_validator_stmt_list.push(
                                parse2(quote! {
                                    if !self.#child_name_ident.validate()? {
                                        return Ok(false);
                                    }
                                })
                                .unwrap(),
                            );
                        }
                        Occurrence::Optional => {
                            children_validator_stmt_list.push(
                                parse2(quote! {
                                    if let Some(#child_name_ident) = &self.#child_name_ident && !#child_name_ident.validate()? {
                                        return Ok(false);
                                    }
                                })
                                .unwrap(),
                            );
                        }
                        Occurrence::Repeated => {
                            children_validator_stmt_list.push(
                                parse2(quote! {
                                    for child in &self.#child_name_ident {
                                        if !child.validate()? {
                                            return Ok(false);
                                        }
                                    }
                                })
                                .unwrap(),
                            );
                        }
                    }
                }
            } else {
                let child_choice_enum_type: Type = parse_str(&format!(
                    "crate::schemas::{}::{}ChildChoice",
                    &schema.module_name,
                    t.class_name.to_upper_camel_case()
                ))
                .unwrap();

                let mut child_match_arm_list: Vec<Arm> = vec![];

                for child in &t.children {
                    let child_name_list: Vec<&str> = child.name.split('/').collect();

                    let child_rename_ser_str = child_name_list
                        .last()
                        .ok_or(format!("{:?}", child.name))
                        .unwrap();

                    let child_variant_name_ident: Ident =
                        parse_str(&child_rename_ser_str.to_upper_camel_case()).unwrap();

                    child_match_arm_list.push(
            parse2(quote! {
              #child_choice_enum_type::#child_variant_name_ident(c) => if !c.validate()? {
                return Ok(false);
              },
            })
            .unwrap(),
          );
                }

                if !t.children.is_empty() {
                    children_validator_stmt_list.push(
                        parse2(quote! {
                          for child in &self.children {
                            match child {
                              #( #child_match_arm_list )*
                            }
                          }
                        })
                        .unwrap(),
                    );
                }
            }
        } else {
            panic!("{t:?}");
        }

        token_stream_list.push(
            parse2(quote! {
              impl #struct_type {
                pub fn validate(&self) -> Result<bool, crate::common::SdkError> {
                  #( #attr_validator_stmt_list )*

                  #( #children_validator_stmt_list )*

                  Ok(true)
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

fn gen_attr_validator_stmt_list(
    OpenXmlSchemaTypeAttribute {
        q_name,
        property_name,
        r#type,
        validators,
        ..
    }: &OpenXmlSchemaTypeAttribute,
    _schema_namespace: &OpenXmlNamespace,
    _gen_context: &GenContext,
) -> Vec<Stmt> {
    let mut attr_validator_stmt_list: Vec<Stmt> = vec![];

    let attr_name_ident_raw = if property_name.is_empty() {
        q_name
    } else {
        property_name
    };

    let attr_name_ident: Ident = parse_str(&escape_snake_case(attr_name_ident_raw)).unwrap();

    let mut required = false;

    for validator in validators {
        if validator.name == "RequiredValidator" {
            required = true;
        }
    }

    let mut validator_count: usize = 0;

    for validator in validators {
        if r#type.starts_with("ListValue<") || r#type.starts_with("EnumValue<") {
            continue;
        }

        match validator.name.as_str() {
            "StringValidator" => {
                let mut add_validator = false;

                for argument in &validator.arguments {
                    match argument.name.as_str() {
                        "MinLength" => {
                            add_validator = true;

                            let value: usize = argument.value.parse().unwrap();

                            if value == 0 {
                                continue;
                            } else if value == 1 {
                                if required {
                                    attr_validator_stmt_list.push(
                                        parse2(quote! {
                                          if self.#attr_name_ident.is_empty() {
                                            validator_results[#validator_count] = false;
                                          }
                                        })
                                        .unwrap(),
                                    );
                                } else {
                                    attr_validator_stmt_list.push(
                                        parse2(quote! {
                                          if #attr_name_ident.is_empty() {
                                            validator_results[#validator_count] = false;
                                          }
                                        })
                                        .unwrap(),
                                    );
                                }
                            } else if required {
                                attr_validator_stmt_list.push(
                                    parse2(quote! {
                                      if self.#attr_name_ident.len() < #value {
                                        validator_results[#validator_count] = false;
                                      }
                                    })
                                    .unwrap(),
                                );
                            } else {
                                attr_validator_stmt_list.push(
                                    parse2(quote! {
                                      if #attr_name_ident.len() < #value {
                                        validator_results[#validator_count] = false;
                                      }
                                    })
                                    .unwrap(),
                                );
                            }
                        }
                        "MaxLength" => {
                            add_validator = true;

                            let value: usize = argument.value.parse().unwrap();

                            if required {
                                attr_validator_stmt_list.push(
                                    parse2(quote! {
                                      if self.#attr_name_ident.len() > #value {
                                        validator_results[#validator_count] = false;
                                      }
                                    })
                                    .unwrap(),
                                );
                            } else {
                                attr_validator_stmt_list.push(
                                    parse2(quote! {
                                      if #attr_name_ident.len() > #value {
                                        validator_results[#validator_count] = false;
                                      }
                                    })
                                    .unwrap(),
                                );
                            }
                        }
                        _ => (),
                    }
                }

                if add_validator {
                    attr_validator_stmt_list.push(
                        parse2(quote! {
                          validator_results[#validator_count] = true;
                        })
                        .unwrap(),
                    );

                    validator_count += 1;
                }
            }
            "NumberValidator" => {
                let mut add_validator = false;

                for argument in &validator.arguments {
                    match argument.name.as_str() {
                        "MinInclusive" => {
                            add_validator = true;

                            let value: i64 = argument.value.parse().unwrap();

                            match r#type.as_str() {
                                "Int64Value" => {
                                    if required {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if self.#attr_name_ident < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if *#attr_name_ident < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    }
                                }
                                "StringValue" | "IntegerValue" | "SByteValue" | "DecimalValue" => {
                                    if required {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if self.#attr_name_ident.parse::<i64>()? < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if #attr_name_ident.parse::<i64>()? < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    }
                                }
                                _ => {
                                    if required {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if (self.#attr_name_ident as i64) < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if (*#attr_name_ident as i64) < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    }
                                }
                            }
                        }
                        "MaxInclusive" => {
                            add_validator = true;

                            let value: i64 = argument.value.parse().unwrap();

                            match r#type.as_str() {
                                "Int64Value" => {
                                    if required {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if self.#attr_name_ident > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if *#attr_name_ident > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    }
                                }
                                "StringValue" | "IntegerValue" | "SByteValue" | "DecimalValue" => {
                                    if required {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if self.#attr_name_ident.parse::<i64>()? > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if #attr_name_ident.parse::<i64>()? > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    }
                                }
                                _ => {
                                    if required {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if (self.#attr_name_ident as i64) > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if (*#attr_name_ident as i64) > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    }
                                }
                            }
                        }
                        _ => (),
                    }
                }

                if add_validator {
                    attr_validator_stmt_list.push(
                        parse2(quote! {
                          validator_results[#validator_count] = true;
                        })
                        .unwrap(),
                    );

                    validator_count += 1;
                }
            }
            _ => (),
        }
    }

    if required && validator_count > 0 {
        let mut stmt_list = vec![
            parse2(quote! {
              let mut validator_results: Vec<bool> = vec![true; #validator_count];
            })
            .unwrap(),
        ];

        stmt_list.extend(attr_validator_stmt_list);

        stmt_list.push(
            parse2(quote! {
              if !validator_results.into_iter().any(|x| x) {
                return Ok(false);
              }
            })
            .unwrap(),
        );

        stmt_list
    } else if validator_count > 0 {
        vec![
            parse2(quote! {
              if let Some(#attr_name_ident) = &self.#attr_name_ident {
                let mut validator_results: Vec<bool> = vec![true; #validator_count];

                #( #attr_validator_stmt_list )*

                if !validator_results.into_iter().any(|x| x) {
                  return Ok(false);
                }
              }
            })
            .unwrap(),
        ]
    } else {
        vec![]
    }
}
