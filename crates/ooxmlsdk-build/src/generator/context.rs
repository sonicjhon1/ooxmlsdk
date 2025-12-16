use heck::ToSnakeCase;
use std::{
    collections::{HashMap, HashSet},
    fs,
    fs::File,
    path::Path,
};

use crate::{
    error::BuildErrorReport,
    models::{
        OpenXmlNamespace, OpenXmlPart, OpenXmlSchema, OpenXmlSchemaEnum, OpenXmlSchemaType,
        TypedNamespace, TypedSchema,
    },
    utils::HashMapOpsError,
};

#[derive(Debug, Default)]
pub struct GenContext<'a> {
    pub parts: Vec<OpenXmlPart>,
    pub schemas: Vec<OpenXmlSchema>,
    pub namespaces: Vec<OpenXmlNamespace>,
    pub typed_namespaces: Vec<TypedNamespace>,
    pub typed_schemas: Vec<Vec<TypedSchema>>,
    pub prefix_namespace_map: HashMap<&'a str, &'a OpenXmlNamespace>,
    pub uri_namespace_map: HashMap<&'a str, &'a OpenXmlNamespace>,
    pub prefix_schema_map: HashMap<&'a str, &'a OpenXmlSchema>,
    pub enum_type_enum_map: HashMap<&'a str, &'a OpenXmlSchemaEnum>,
    pub enum_type_namespace_map: HashMap<&'a str, &'a OpenXmlNamespace>,
    pub type_name_type_map: HashMap<&'a str, &'a OpenXmlSchemaType>,
    pub type_name_namespace_map: HashMap<&'a str, &'a OpenXmlNamespace>,
    pub namespace_typed_namespace_map: HashMap<&'a str, &'a TypedNamespace>,
    pub part_name_type_name_map: HashMap<&'a str, &'a str>,
}

impl<'a> GenContext<'a> {
    pub(crate) fn new(data_dir: impl AsRef<Path>) -> Self {
        let data_dir = data_dir.as_ref();
        let data_parts_dir_path = &data_dir.join("parts");
        let data_schemas_dir_path = &data_dir.join("schemas");
        let data_typed_dir_path = &data_dir.join("typed");

        let mut parts: Vec<OpenXmlPart> = vec![];
        let mut schemas: Vec<OpenXmlSchema> = vec![];
        let mut typed_schemas: Vec<Vec<TypedSchema>> = vec![];

        for entry in fs::read_dir(data_parts_dir_path).unwrap() {
            let entry = entry.unwrap();

            let file = File::open(entry.path()).unwrap();

            let mut open_xml_part: OpenXmlPart = serde_json::from_reader(file).unwrap();

            let part_mod = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_snake_case();

            open_xml_part.module_name = part_mod;

            parts.push(open_xml_part);
        }

        for entry in fs::read_dir(data_schemas_dir_path).unwrap() {
            let entry = entry.unwrap();

            let file = File::open(entry.path()).unwrap();

            let mut open_xml_schema: OpenXmlSchema = serde_json::from_reader(file).unwrap();

            let schema_mod = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_snake_case();

            open_xml_schema.module_name = schema_mod;

            schemas.push(open_xml_schema);
        }

        for entry in fs::read_dir(data_typed_dir_path).unwrap() {
            let entry = entry.unwrap();

            if entry.file_name().to_string_lossy() != "namespaces.json" {
                let file = File::open(entry.path()).unwrap();

                let typed_schema: Vec<TypedSchema> = serde_json::from_reader(file).unwrap();

                typed_schemas.push(typed_schema);
            }
        }

        let file = File::open(data_dir.join("namespaces.json")).unwrap();

        let namespaces: Vec<OpenXmlNamespace> = serde_json::from_reader(file).unwrap();

        let file = File::open(data_dir.join("typed").join("namespaces.json")).unwrap();

        let typed_namespaces: Vec<TypedNamespace> = serde_json::from_reader(file).unwrap();

        let mut part_name_version_map: HashMap<String, String> =
            HashMap::with_capacity(parts.len());
        let mut part_name_part_map: HashMap<String, &OpenXmlPart> =
            HashMap::with_capacity(parts.len());

        for part in parts.iter() {
            part_name_version_map.insert(part.name.clone(), part.version.clone());
            part_name_part_map.insert(part.name.clone(), part);
        }

        let mut uri_namespace_version_map: HashMap<&str, &str> = HashMap::new();

        for namespace in namespaces.iter() {
            uri_namespace_version_map.insert(&namespace.uri, &namespace.version);
        }

        let mut type_name_version_map: HashMap<String, String> = HashMap::new();

        for schema in schemas.iter() {
            for schema_type in schema.types.iter() {
                type_name_version_map.insert(schema_type.name.clone(), schema_type.version.clone());
            }
        }

        let mut part_type_name_map: HashMap<&str, &str> = HashMap::new();

        for typed_schema_list in typed_schemas.iter() {
            for typed_schema in typed_schema_list.iter() {
                if !typed_schema.part_class_name.is_empty() {
                    part_type_name_map.insert(&typed_schema.part_class_name, &typed_schema.name);
                }
            }
        }

        #[allow(unused_mut)]
        let mut part_name_set: HashSet<String> = HashSet::new();

        #[cfg(feature = "docx")]
        gen_part_name_set(
            &mut part_name_set,
            "WordprocessingDocument",
            &part_name_part_map,
        )
        .unwrap();

        #[cfg(feature = "xlsx")]
        gen_part_name_set(
            &mut part_name_set,
            "SpreadsheetDocument",
            &part_name_part_map,
        )
        .unwrap();

        #[cfg(feature = "pptx")]
        gen_part_name_set(
            &mut part_name_set,
            "PresentationDocument",
            &part_name_part_map,
        )
        .unwrap();

        parts.retain(|x| {
            if !part_name_set.contains(&x.name) {
                return false;
            }

            if let Some(part_type_name) = part_type_name_map.get(x.name.as_str()) {
                let type_version = type_name_version_map.try_get(*part_type_name).unwrap();

                check_office_version(&x.version) && check_office_version(type_version)
            } else {
                check_office_version(&x.version)
            }
        });

        for part in parts.iter_mut() {
            part.children.retain(|x| {
                if x.is_data_part_reference {
                    return true;
                }

                let child_version = part_name_version_map.try_get(&x.name).unwrap();

                if let Some(part_type_name) = part_type_name_map.get(x.name.as_str()) {
                    let type_version = type_name_version_map.try_get(*part_type_name).unwrap();

                    check_office_version(child_version) && check_office_version(type_version)
                } else {
                    check_office_version(child_version)
                }
            });
        }

        for schema in schemas.iter_mut() {
            for schema_type in schema.types.iter_mut() {
                schema_type.module_name = schema.module_name.clone();
            }

            for schema_enum in schema.enums.iter_mut() {
                schema_enum.module_name = schema.module_name.clone();
            }
        }

        let mut type_name_set: HashSet<String> = HashSet::new();

        let mut type_name_type_map: HashMap<String, &OpenXmlSchemaType> = HashMap::new();

        for schema in schemas.iter() {
            for ty in schema.types.iter() {
                type_name_type_map.insert(ty.name.clone(), ty);
            }
        }

        for part in parts.iter() {
            if part.base == "OpenXmlPart" && !part.root.is_empty() {
                let type_name = part_type_name_map.try_get(part.name.as_str()).unwrap();

                gen_type_name_set(&mut type_name_set, type_name, &type_name_type_map).unwrap()
            }
        }

        for schema in schemas.iter_mut() {
            for schema_enum in schema.enums.iter_mut() {
                schema_enum
                    .facets
                    .retain(|x| check_office_version(&x.version));
            }

            schema.enums.retain(|x| check_office_version(&x.version));

            for schema_type in schema.types.iter_mut() {
                schema_type
                    .attributes
                    .retain(|x| check_office_version(&x.version));

                schema_type.children.retain(|x| {
                    let child_type_version =
                        type_name_version_map.try_get_mut(x.name.as_str()).unwrap();

                    check_office_version(child_type_version)
                });

                schema_type.particle.check_particle_version();
            }

            schema.types.retain(|x| check_office_version(&x.version));
        }

        schemas.retain(|x| {
            let schema_namespace_version = uri_namespace_version_map
                .try_get(x.target_namespace.as_str())
                .unwrap();

            check_office_version(schema_namespace_version)
        });

        Self {
            parts,
            schemas,
            namespaces,
            typed_schemas,
            typed_namespaces,
            ..Default::default()
        }
    }
}

pub(crate) fn gen_type_name_set(
    type_name_set: &mut HashSet<String>,
    type_name: &str,
    type_name_type_map: &HashMap<String, &OpenXmlSchemaType>,
) -> Result<(), BuildErrorReport> {
    if type_name_set.insert(type_name.to_string()) {
        let schema_type = type_name_type_map.try_get(type_name)?;

        if schema_type.is_derived {
            let (type_base_class, _) = schema_type.split_name();
            // TODO: Remove this
            debug_assert_eq!(
                format!("{type_base_class}/"),
                type_name[00..type_name.find('/').unwrap() + 1].to_string()
            );
            type_name_set.insert(format!("{type_base_class}/"));
        }

        for type_child in schema_type.children.iter() {
            gen_type_name_set(type_name_set, &type_child.name, type_name_type_map)?;
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub(crate) fn gen_part_name_set(
    part_name_set: &mut HashSet<String>,
    part_name: &str,
    part_name_part_map: &HashMap<String, &OpenXmlPart>,
) -> Result<(), BuildErrorReport> {
    if part_name_set.insert(part_name.to_string()) {
        let part = part_name_part_map.try_get(part_name).unwrap();

        for part_child in part.children.iter() {
            if part_child.is_data_part_reference {
                continue;
            }

            gen_part_name_set(part_name_set, &part_child.name, part_name_part_map)?;
        }
    }

    Ok(())
}

pub(crate) fn check_office_version(version: &str) -> bool {
    match version {
        #[cfg(feature = "microsoft365")]
        "Microsoft365" => true,
        #[cfg(not(feature = "microsoft365"))]
        "Microsoft365" => false,
        #[cfg(feature = "office2021")]
        "Office2021" => true,
        #[cfg(not(feature = "office2021"))]
        "Office2021" => false,
        #[cfg(feature = "office2019")]
        "Office2019" => true,
        #[cfg(not(feature = "office2019"))]
        "Office2019" => false,
        #[cfg(feature = "office2016")]
        "Office2016" => true,
        #[cfg(not(feature = "office2016"))]
        "Office2016" => false,
        #[cfg(feature = "office2013")]
        "Office2013" => true,
        #[cfg(not(feature = "office2013"))]
        "Office2013" => false,
        #[cfg(feature = "office2010")]
        "Office2010" => true,
        #[cfg(not(feature = "office2010"))]
        "Office2010" => false,
        "Office2007" => true,
        "" => true,
        _ => false,
    }
}
