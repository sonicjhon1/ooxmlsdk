use heck::ToUpperCamelCase;
use quote::quote;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use syn::{Arm, Ident, Stmt, Type, parse_str, parse2};

use crate::{
    GenContext,
    error::*,
    models::{
        Occurrence, OpenXmlSchema, OpenXmlSchemaType, OpenXmlSchemaTypeAttribute,
        OpenXmlSchemaTypeChild,
    },
    utils::HashMapOpsError,
};

pub fn gen_validators(
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

    let struct_type: Type = parse_str(&format!(
        "crate::schemas::{}::{}",
        &schema.module_name,
        schema_type.class_name.to_upper_camel_case()
    ))
    .unwrap();

    let (type_base_class, _) = schema_type.split_name();

    let mut attr_validator_stmt_list: Vec<Stmt> = vec![];

    let mut children_validator_stmt_list: Vec<Stmt> = vec![];

    for attr in &schema_type.attributes {
        attr_validator_stmt_list.extend(gen_attr_validator_stmt_list(attr));
    }

    if schema_type.base_class == "OpenXmlLeafTextElement"
        || schema_type.base_class == "OpenXmlLeafElement"
    {
    } else if schema_type.base_class == "OpenXmlCompositeElement"
        || schema_type.base_class == "CustomXmlElement"
        || schema_type.base_class == "OpenXmlPartRootElement"
        || schema_type.base_class == "SdtElement"
    {
        if schema_type.is_one_sequence_flatten() {
            let mut child_map: HashMap<&str, &OpenXmlSchemaTypeChild> = HashMap::new();

            for child in &schema_type.children {
                child_map.insert(&child.name, child);
            }

            for schema_type_particle in &schema_type.particle.items {
                let child = child_map.try_get(schema_type_particle.name.as_str())?;
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
                schema_type.class_name.to_upper_camel_case()
            ))
            .map_err(BuildError::from)?;

            let mut child_match_arm_list: Vec<Arm> = vec![];

            for child in &schema_type.children {
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

            if !schema_type.children.is_empty() {
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
    } else if schema_type.is_derived {
        let base_class_type = gen_context
            .type_name_type_map
            .try_get(format!("{type_base_class}/").as_str())?;

        for attr in &base_class_type.attributes {
            attr_validator_stmt_list.extend(gen_attr_validator_stmt_list(attr));
        }

        if schema_type.is_one_sequence_flatten() && base_class_type.composite_type == "OneSequence"
        {
            let mut child_map: HashMap<&str, &OpenXmlSchemaTypeChild> = HashMap::new();

            for child in &schema_type.children {
                child_map.insert(&child.name, child);
            }

            for schema_type_particle in &schema_type.particle.items {
                let child = child_map.try_get(schema_type_particle.name.as_str())?;
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
                schema_type.class_name.to_upper_camel_case()
            ))
            .unwrap();

            let mut child_match_arm_list: Vec<Arm> = vec![];

            for child in &schema_type.children {
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

            if !schema_type.children.is_empty() {
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
        panic!("{schema_type:?}");
    }

    return Ok(quote! {
      impl #struct_type {
        pub fn validate(&self) -> Result<bool, crate::common::SdkErrorReport> {
          #( #attr_validator_stmt_list )*

          #( #children_validator_stmt_list )*

          Ok(true)
        }
      }
    }
    .to_string());
}

fn gen_attr_validator_stmt_list(schema: &OpenXmlSchemaTypeAttribute) -> Vec<Stmt> {
    let mut attr_validator_stmt_list: Vec<Stmt> = vec![];

    let attr_name_ident = schema.as_name_ident();

    let required = schema.is_validator_required();

    let mut validator_count: usize = 0;

    for validator in &schema.validators {
        if schema.r#type.starts_with("ListValue<") || schema.r#type.starts_with("EnumValue<") {
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

                            match schema.r#type.as_str() {
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
                                              if self.#attr_name_ident.parse::<i64>().map_err(crate::common::SdkError::from)? < #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if #attr_name_ident.parse::<i64>().map_err(crate::common::SdkError::from)? < #value {
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

                            match schema.r#type.as_str() {
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
                                              if self.#attr_name_ident.parse::<i64>().map_err(crate::common::SdkError::from)? > #value {
                                                validator_results[#validator_count] = false;
                                              }
                                            })
                                            .unwrap(),
                                        );
                                    } else {
                                        attr_validator_stmt_list.push(
                                            parse2(quote! {
                                              if #attr_name_ident.parse::<i64>().map_err(crate::common::SdkError::from)? > #value {
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
