use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::TokenStream;
use quote::quote;
use rootcause::prelude::ResultExt;
use syn::{
    Arm, FieldValue, Ident, ImplItemConst, ItemFn, ItemImpl, ItemStruct, Stmt, Type, parse_quote,
    parse_str, parse2,
};

use crate::{
    GenContext,
    error::{BuildError, BuildErrorReport},
    models::{Occurrence, OpenXmlPart},
    utils::{HashMapOpsError, gen_use_common_glob},
};

pub fn gen_open_xml_parts(
    part: &OpenXmlPart,
    gen_context: &GenContext,
) -> Result<TokenStream, BuildErrorReport> {
    let use_common_glob = gen_use_common_glob();

    let relationship_type_str = &part.relationship_type;
    let relationship_type_impl_const: ImplItemConst = parse_quote! {
        pub const RELATIONSHIP: &str = #relationship_type_str;
    };

    let part_name_raw = part.name.as_str();
    let part_struct_name_ident: Ident = parse_str(&part_name_raw.to_upper_camel_case()).unwrap();
    let part_struct = gen_struct_fn(part, gen_context, &part_struct_name_ident)?;

    let path_str = if part.paths.general.is_empty() {
        ""
    } else {
        &format!("{}/", part.paths.general)
    };

    let mut field_declaration_list: Vec<Stmt> = vec![];
    let mut field_unwrap_list: Vec<Stmt> = vec![];
    let mut self_field_value_list: Vec<FieldValue> = vec![];
    let mut children_stmt: Option<Stmt> = None;
    let mut children_arm_list: Vec<Arm> = vec![];

    let part_rels_path_ident: Ident =
        parse_str(&format!("{}_rels_path", part_name_raw.to_snake_case())).unwrap();

    if part.base == "OpenXmlPackage" {
        field_declaration_list.push(
            parse2(quote! {
              let content_types = crate::common::opc_content_types::Types::from_reader(
                std::io::BufReader::new(archive.by_name("[Content_Types].xml").map_err(SdkError::from)?,
              ))?;
            })
            .unwrap(),
        );

        self_field_value_list.push(
            parse2(quote! {
              content_types
            })
            .unwrap(),
        );
    } else {
        self_field_value_list.push(
            parse2(quote! {
              r_id: r_id.to_string()
            })
            .unwrap(),
        );
    }

    if !part.children.is_empty() {
        field_declaration_list.push(
            parse2(quote! {
              let mut rels_path = "".to_string();
            })
            .unwrap(),
        );

        field_declaration_list.push(
            parse2(quote! {
              let child_parent_path = format!("{}{}", parent_path, #path_str);
            })
            .unwrap(),
        );

        field_declaration_list.push(
            parse2(quote! {
              let part_target_str = if path.ends_with(".xml") {
                &path
                  .rsplit_once('/')
                  .ok_or_else(|| SdkError::CommonError(path.to_string()))?
                  .1
              } else {
                ""
              };
            })
            .unwrap(),
        );

        field_declaration_list.push(
            parse2(quote! {
              let #part_rels_path_ident = resolve_zip_file_path(
                &format!("{child_parent_path}_rels/{part_target_str}.rels"),
              );
            })
            .unwrap(),
        );

        field_declaration_list.push(
            parse2(quote! {
                let relationships = if let Some(file_path) = file_path_set.get(&#part_rels_path_ident) {
                    rels_path = file_path.to_string();

                    Some(crate::common::opc_relationships::Relationships::from_reader(
                        std::io::BufReader::new(archive.by_name(file_path).map_err(SdkError::from)?)
                    )?)
                } else {
                  None
                };
            })
            .unwrap(),
        );

        self_field_value_list.push(
            parse2(quote! {
                rels_path
            })
            .unwrap(),
        );

        self_field_value_list.push(
            parse2(quote! {
                relationships
            })
            .unwrap(),
        );
    }

    self_field_value_list.push(
        parse2(quote! {
            inner_path: path.to_string()
        })
        .unwrap(),
    );

    match (part_name_raw, !part.extension.is_empty()) {
        ("CustomXmlPart" | "XmlSignaturePart", _) => {
            field_declaration_list.push(
                parse2(quote! {
                    use std::io::Read;
                })
                .unwrap(),
            );

            field_declaration_list.push(
                parse2(quote! {
                    let mut part_content = String::new();
                })
                .unwrap(),
            );

            field_declaration_list.push(
                parse2(quote! {
                    {
                        let mut file = std::io::BufReader::new(archive.by_name(path).map_err(SdkError::from)?);
                        file.read_to_string(&mut part_content).map_err(SdkError::from)?;
                    }
                })
                .unwrap(),
            );

            self_field_value_list.push(
                parse2(quote! {
                    part_content
                })
                .unwrap(),
            );
        }
        ("CustomDataPart" | "InternationalMacroSheetPart", _) | (_, true) => {
            field_declaration_list.push(
                parse2(quote! {
                    use std::io::Read;
                })
                .unwrap(),
            );

            field_declaration_list.push(
                parse2(quote! {
                    let mut part_content;
                })
                .unwrap(),
            );

            field_declaration_list.push(
                parse2(quote! {
                    {
                        let mut zip_entry = archive.by_name(path).map_err(SdkError::from)?;

                        part_content = Vec::with_capacity(zip_entry.size() as usize);

                        zip_entry.read_to_end(&mut part_content).map_err(SdkError::from)?;
                    }
                })
                .unwrap(),
            );

            self_field_value_list.push(
                parse2(quote! {
                    part_content
                })
                .unwrap(),
            );
        }
        ("CoreFilePropertiesPart", _) => {
            field_declaration_list.push(
                parse2(quote! {
                    let root_element = Some(
                        crate::common::opc_core_properties::CoreProperties::from_reader(
                            std::io::BufReader::new(archive.by_name(path).map_err(SdkError::from)?)
                        )?,
                    );
                })
                .unwrap(),
            );

            field_unwrap_list.push(
                parse2(quote! {
                    let root_element = root_element
                        .ok_or_else(|| SdkError::CommonError("root_element".to_string()))?;
                })
                .unwrap(),
            );

            self_field_value_list.push(
                parse2(quote! {
                    root_element
                })
                .unwrap(),
            );
        }
        _ => {
            if let Some(root_element_type_name) =
                gen_context.part_name_type_name_map.get(part_name_raw)
            {
                let root_element_type = gen_context
                    .type_name_type_map
                    .try_get(root_element_type_name)?;

                let field_type: Type = parse_str(&format!(
                    "crate::schemas::{}::{}",
                    root_element_type.module_name,
                    root_element_type.class_name.to_upper_camel_case()
                ))
                .unwrap();

                field_declaration_list.push(
                    parse2(quote! {
                        let root_element = Some(#field_type::from_reader(
                            std::io::BufReader::new(archive.by_name(path).map_err(SdkError::from)?)
                        )?);
                    })
                    .unwrap(),
                );

                field_unwrap_list.push(
                    parse2(quote! {
                        let root_element = root_element
                            .ok_or_else(|| SdkError::CommonError("root_element".to_string()))?;
                    })
                    .unwrap(),
                );

                self_field_value_list.push(
                    parse2(quote! {
                        root_element
                    })
                    .unwrap(),
                );
            }
        }
    }

    for child in &part.children {
        if child.is_data_part_reference {
            continue;
        }

        let child_api_name_str = child.api_name.to_snake_case();
        let child_api_name_ident: Ident = parse_str(&child_api_name_str).unwrap();

        let child_name_struct_str = child.name.to_upper_camel_case();
        let child_name_str = child.name.to_snake_case();
        let child_name_ident: Ident = parse_str(&child_name_str).unwrap();

        let child_type: Type = parse_str(&format!(
            "crate::parts::{child_name_str}::{child_name_struct_str}",
        ))
        .unwrap();

        let relationship_type_ty: Type = parse_str(&format!(
            "crate::parts::{child_name_str}::{child_name_struct_str}::RELATIONSHIP",
        ))
        .unwrap();

        if child.max_occurs_great_than_one {
            field_declaration_list.push(
                parse2(quote! {
                    let mut #child_api_name_ident: Vec<#child_type> = vec![];
                })
                .unwrap(),
            );

            children_arm_list.push(
                parse2(quote! {
                    #relationship_type_ty => {
                        let target_path = resolve_zip_file_path(
                            &format!("{}{}", child_parent_path, relationship.target),
                        );

                        let #child_name_ident = #child_type::new_from_archive(
                            &child_parent_path,
                            &target_path,
                            &relationship.id,
                            file_path_set,
                            archive,
                        )?;

                        #child_api_name_ident.push(#child_name_ident);
                    }
                })
                .unwrap(),
            );
        } else {
            field_declaration_list.push(
                parse2(quote! {
                    let mut #child_api_name_ident: Option<std::boxed::Box<#child_type>> = None;
                })
                .unwrap(),
            );

            children_arm_list.push(
                parse2(quote! {
                    #relationship_type_ty => {
                        let target_path = resolve_zip_file_path(
                            &format!("{}{}", child_parent_path, relationship.target),
                        );

                        #child_api_name_ident = Some(std::boxed::Box::new(#child_type::new_from_archive(
                            &child_parent_path,
                            &target_path,
                            &relationship.id,
                            file_path_set,
                            archive,
                        )?));
                    }
                })
                .unwrap(),
            );

            if child.min_occurs_is_non_zero {
                field_unwrap_list.push(
                    parse2(quote! {
                        let #child_api_name_ident = #child_api_name_ident
                            .ok_or_else(|| SdkError::CommonError(#child_api_name_str.to_string()))?;
                    })
                    .unwrap(),
                );
            }
        }

        self_field_value_list.push(
            parse2(quote! {
              #child_api_name_ident
            })
            .unwrap(),
        );
    }

    if !part.children.is_empty() {
        children_stmt = Some(
            parse2(quote! {
                if let Some(relationships) = &relationships {
                    for relationship in &relationships.relationship {
                        #[allow(clippy::single_match)]
                        match relationship.r#type.as_str() {
                            #( #children_arm_list, )*
                            _ => ()
                        }
                    }
                }
            })
            .unwrap(),
        );
    }

    let part_new_from_archive_fn = gen_from_archive_fn(
        field_declaration_list,
        children_stmt,
        field_unwrap_list,
        self_field_value_list,
    )?;

    let part_save_zip_fn = gen_save_zip_fn(part, gen_context, path_str)?;

    let part_impl: ItemImpl = if part.base == "OpenXmlPackage" {
        let part_new_fn: ItemFn = parse2(quote! {
            pub fn new<R: std::io::Read + std::io::Seek>(
                reader: R,
            ) -> Result<Self, SdkErrorReport> {
                let mut archive = zip::ZipArchive::new(reader).map_err(SdkError::from)?;
                let mut file_path_set = std::collections::HashSet::with_capacity(archive.len());

                for i in 0..archive.len() {
                    let file = archive.by_index(i).map_err(SdkError::from)?;
                    if let Some(path) = file.enclosed_name() {
                        file_path_set.insert(path.to_string_lossy().into_owned());
                    }
                }

                Self::new_from_archive("", "", "", &file_path_set, &mut archive)
            }
        })
        .unwrap();

        let part_new_from_file_fn: ItemFn = parse2(quote! {
            pub fn new_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, SdkErrorReport> {
                Self::new(std::io::BufReader::new(std::fs::File::open(path).map_err(SdkError::from)?))
            }
        })
        .unwrap();

        let part_save_fn: ItemFn = parse2(quote! {
            pub fn save<W: std::io::Write + std::io::Seek>(&self, writer: W) -> Result<(), SdkErrorReport> {
                use std::io::Write;

                let mut entry_set: std::collections::HashSet<String> = std::collections::HashSet::new();

                let mut zip = zip::ZipWriter::new(writer);

                let options = zip::write::SimpleFileOptions::default()
                  .compression_method(zip::CompressionMethod::Deflated)
                  .unix_permissions(0o755);

                zip.start_file("[Content_Types].xml", options).map_err(SdkError::from)?;

                zip.write_all(&self.content_types.to_xml_bytes(true, false)).map_err(SdkError::from)?;

                self.save_zip("", &mut zip, &mut entry_set)?;

                zip.finish().map_err(SdkError::from)?;

                Ok(())
            }
        })
        .unwrap();

        let part_save_to_file_fn: ItemFn = parse2(quote! {
            pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), SdkErrorReport> {
                self.save(std::fs::File::create(path).map_err(SdkError::from)?)
            }
        })
        .unwrap();

        parse_quote! {
            impl #part_struct_name_ident {
                #relationship_type_impl_const

                #part_new_fn

                #part_new_from_file_fn

                #part_new_from_archive_fn

                #part_save_fn

                #part_save_to_file_fn

                #part_save_zip_fn
            }
        }
    } else {
        parse_quote! {
            impl #part_struct_name_ident {
                #relationship_type_impl_const

                #part_new_from_archive_fn

                #part_save_zip_fn
            }
        }
    };

    Ok(quote! {
        #use_common_glob

        #part_struct

        #part_impl
    })
}

fn gen_struct_fn(
    part: &OpenXmlPart,
    gen_context: &GenContext,
    struct_name_ident: &Ident,
) -> Result<ItemStruct, BuildErrorReport> {
    let part_name_raw = part.name.as_str();

    let mut fields: Vec<TokenStream> = vec![];

    if part.base == "OpenXmlPackage" {
        fields.push(quote! {
            pub content_types: crate::common::opc_content_types::Types,
        });
    } else {
        fields.push(quote! {
            pub r_id: String,
        });
    }

    if !part.children.is_empty() {
        fields.push(quote! {
            pub relationships: Option<crate::common::opc_relationships::Relationships>,
        });

        fields.push(quote! {
            pub rels_path: String,
        });
    }

    fields.push(quote! {
        pub inner_path: String,
    });

    fields.push(match (part_name_raw, !part.extension.is_empty()) {
        ("CustomXmlPart" | "XmlSignaturePart", _) => quote! {
            pub part_content: String,
        },
        ("CustomDataPart" | "InternationalMacroSheetPart", _) | (_, true) => quote! {
            pub part_content: Vec<u8>,
        },
        ("CoreFilePropertiesPart", _) => quote! {
            pub root_element: crate::common::opc_core_properties::CoreProperties,
        },
        _ => {
            if let Some(root_element_type_name) =
                gen_context.part_name_type_name_map.get(part_name_raw)
            {
                let root_element_type = gen_context
                    .type_name_type_map
                    .try_get(root_element_type_name)?;

                let field_type: Type = parse_str(&format!(
                    "crate::schemas::{}::{}",
                    root_element_type.module_name,
                    root_element_type.class_name.to_upper_camel_case()
                ))
                .unwrap();

                quote! {
                    pub root_element: #field_type,
                }
            } else {
                quote! {}
            }
        }
    });

    for child in &part.children {
        if child.is_data_part_reference {
            continue;
        }

        let child_name_ident: Ident = parse_str(&child.api_name.to_snake_case()).unwrap();

        let child_type: Type = parse_str(&format!(
            "crate::parts::{}::{}",
            child.name.to_snake_case(),
            child.name.to_upper_camel_case(),
        ))
        .unwrap();

        fields.push(match child.as_occurrence() {
            Occurrence::Required => quote! {
                pub #child_name_ident: std::boxed::Box<#child_type>,
            },
            Occurrence::Optional => quote! {
                pub #child_name_ident: Option<std::boxed::Box<#child_type>>,
            },
            Occurrence::Repeated => quote! {
                pub #child_name_ident: Vec<#child_type>,
            },
        });
    }

    parse2(quote! {
        #[derive(Clone, Debug, Default)]
        pub struct #struct_name_ident {
            #( #fields )*
        }
    })
    .context_transform(BuildError::from)
}

fn gen_from_archive_fn(
    field_declaration_list: Vec<Stmt>,
    children_stmt: Option<Stmt>,
    field_unwrap_list: Vec<Stmt>,
    self_field_value_list: Vec<FieldValue>,
) -> Result<ItemFn, BuildErrorReport> {
    parse2(quote! {
        #[allow(unused_variables)]
        pub(crate) fn new_from_archive<R: std::io::Read + std::io::Seek>(
            parent_path: &str,
            path: &str,
            r_id: &str,
            file_path_set: &std::collections::HashSet<String>,
            archive: &mut zip::ZipArchive<R>,
        ) -> Result<Self, SdkErrorReport> {
            #( #field_declaration_list )*

            #children_stmt

            #( #field_unwrap_list )*

            Ok(Self {
                #( #self_field_value_list, )*
            })
        }
    })
    .context_transform(BuildError::from)
}

fn gen_save_zip_fn(
    part: &OpenXmlPart,
    gen_context: &GenContext,
    path_str: &str,
) -> Result<ItemFn, BuildErrorReport> {
    let part_paths_general = &part.paths.general;

    let part_name_raw = part.name.as_str();
    let part_name_dir_path_ident: Ident =
        parse_str(&format!("{part_name_raw}_dir_path").to_snake_case())
            .context_transform(BuildError::from)?;

    let mut writer_list: Vec<TokenStream> = vec![];

    writer_list.push(
        quote! {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755);

            let directory_path = resolve_zip_file_path(parent_path);

            if !directory_path.is_empty() && !entry_set.contains(&directory_path) {
                zip.add_directory(&directory_path, options).map_err(SdkError::from)?;

                entry_set.insert(directory_path);
            }

            let #part_name_dir_path_ident = resolve_zip_file_path(
                &format!("{}{}/", parent_path, #part_paths_general),
            );

            if !#part_name_dir_path_ident.is_empty() && !entry_set.contains(&#part_name_dir_path_ident) {
                zip.add_directory(&#part_name_dir_path_ident, options).map_err(SdkError::from)?;

                entry_set.insert(#part_name_dir_path_ident);
            }
        },
    );

    writer_list.push(
        match (
            part_name_raw,
            !part.extension.is_empty(),
            gen_context
                .part_name_type_name_map
                .contains_key(part_name_raw),
        ) {
            ("CustomXmlPart" | "XmlSignaturePart", _, _) => quote! {
                use std::io::Write;

                if !entry_set.contains(&self.inner_path) {
                    zip.start_file(&self.inner_path, options).map_err(SdkError::from)?;

                    zip.write_all(self.part_content.as_bytes()).map_err(SdkError::from)?;

                    entry_set.insert(self.inner_path.to_string());
                }
            },
            ("CustomDataPart" | "InternationalMacroSheetPart", _, _) | (_, true, _) => quote! {
                use std::io::Write;

                if !entry_set.contains(&self.inner_path) {
                    zip.start_file(&self.inner_path, options).map_err(SdkError::from)?;

                    zip.write_all(&self.part_content).map_err(SdkError::from)?;

                    entry_set.insert(self.inner_path.to_string());
                }
            },
            ("CoreFilePropertiesPart", _, _) | (_, _, true) => quote! {
                use std::io::Write;

                if !entry_set.contains(&self.inner_path) {
                    zip.start_file(&self.inner_path, options).map_err(SdkError::from)?;

                    zip.write_all(&self.root_element.to_xml_bytes(true, false)).map_err(SdkError::from)?;

                    entry_set.insert(self.inner_path.to_string());
                }
            },
            _ if !part.children.is_empty() => quote! {
                use std::io::Write;
            },
            _ => quote! {},
        },
    );

    if !part.children.is_empty() {
        writer_list.push(quote! {
            let child_parent_path = format!("{}{}", parent_path, #path_str);

            if let Some(relationships) = &self.relationships {
                let rels_dir_path = resolve_zip_file_path(
                    &format!("{child_parent_path}_rels"),
                );

                if !rels_dir_path.is_empty() && !entry_set.contains(&rels_dir_path) {
                    zip.add_directory(&rels_dir_path, options).map_err(SdkError::from)?;

                    entry_set.insert(rels_dir_path);
                }

                if !entry_set.contains(&self.rels_path) {
                    zip.start_file(&self.rels_path, options).map_err(SdkError::from)?;

                    zip.write_all(&relationships.to_xml_bytes(true, false)).map_err(SdkError::from)?;

                    entry_set.insert(self.rels_path.to_string());
                }
            }
        });
    }

    let mut children_writer_stmt_list: Vec<Stmt> = vec![];
    for child in &part.children {
        if child.is_data_part_reference {
            continue;
        }

        let child_api_name_ident: Ident =
            parse_str(&child.api_name.to_snake_case()).context_transform(BuildError::from)?;

        let tokens = match child.as_occurrence() {
            Occurrence::Required => quote! {
                self.#child_api_name_ident.save_zip(&child_parent_path, zip, entry_set)?;
            },
            Occurrence::Optional => quote! {
                if let Some(#child_api_name_ident) = &self.#child_api_name_ident {
                    #child_api_name_ident.save_zip(&child_parent_path, zip, entry_set)?;
                }
            },
            Occurrence::Repeated => {
                let child_name_ident: Ident =
                    parse_str(&child.name.to_snake_case()).context_transform(BuildError::from)?;

                quote! {
                    for #child_name_ident in &self.#child_api_name_ident {
                        #child_name_ident.save_zip(&child_parent_path, zip, entry_set)?;
                    }
                }
            }
        };

        children_writer_stmt_list.push(parse2(tokens).map_err(BuildError::from)?);
    }

    parse2(quote! {
        pub(crate) fn save_zip<W: std::io::Write + std::io::Seek>(
            &self,
            parent_path: &str,
            zip: &mut zip::ZipWriter<W>,
            entry_set: &mut std::collections::HashSet<String>,
        ) -> Result<(), SdkErrorReport> {
            #( #writer_list )*

            #( #children_writer_stmt_list )*

            Ok(())
        }
    })
    .context_transform(BuildError::from)
}
