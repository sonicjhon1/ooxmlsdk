#![feature(trim_prefix_suffix)]

use quote::{ToTokens, quote};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{fs, path::Path};
use syn::{Ident, ItemMod, parse_quote, parse_str};

use crate::{
    error::*,
    generator::{
        context::GenContext, deserializer::gen_deserializers,
        open_xml_schema::gen_open_xml_schemas, serializer::gen_serializer,
    },
    utils::HashMapOpsError,
};

pub mod error;
pub mod generator;
pub mod includes;
pub mod models;
pub mod utils;

pub fn generate(out_dir: impl AsRef<Path>) -> Result<(), BuildErrorReport> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    generate_with(crate_root.join("./data/"), out_dir)
}

pub fn generate_with(
    data_dir: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> Result<(), BuildErrorReport> {
    let data_dir = data_dir.as_ref();
    let out_dir = out_dir.as_ref();

    let mut gen_context = GenContext::new(data_dir);

    for namespace in gen_context.namespaces.iter() {
        gen_context
            .prefix_namespace_map
            .insert(&namespace.prefix, namespace);

        gen_context
            .uri_namespace_map
            .insert(&namespace.uri, namespace);
    }

    for typed_namespace in gen_context.typed_namespaces.iter() {
        gen_context
            .namespace_typed_namespace_map
            .insert(&typed_namespace.namespace, typed_namespace);
    }

    for typed_schemas in gen_context.typed_schemas.iter() {
        for typed_schema in typed_schemas.iter() {
            if !typed_schema.part_class_name.is_empty() {
                gen_context
                    .part_name_type_name_map
                    .insert(&typed_schema.part_class_name, &typed_schema.name);
            }
        }
    }

    for schema in gen_context.schemas.iter() {
        let namespace = gen_context
            .uri_namespace_map
            .try_get(schema.target_namespace.as_str())?;

        gen_context
            .prefix_schema_map
            .insert(&namespace.prefix, schema);

        for schema_enum in schema.enums.iter() {
            gen_context
                .enum_type_enum_map
                .insert(&schema_enum.r#type, schema_enum);

            gen_context
                .enum_type_namespace_map
                .insert(&schema_enum.r#type, namespace);
        }

        for schema_type in schema.types.iter() {
            gen_context
                .type_name_type_map
                .insert(&schema_type.name, schema_type);

            gen_context
                .type_name_namespace_map
                .insert(&schema_type.name, namespace);

            if !schema_type.part.is_empty() {
                gen_context
                    .part_name_type_name_map
                    .insert(&schema_type.part, &schema_type.name);
            }
        }
    }

    gen_context
        .part_name_type_name_map
        .insert("StyleDefinitionsPart", "w:CT_Styles/w:styles");
    gen_context
        .part_name_type_name_map
        .insert("StylesWithEffectsPart", "w:CT_Styles/w:styles");

    [
        write_common,
        write_schemas,
        write_deserializers,
        write_serializers,
        #[cfg(feature = "parts")]
        write_parts,
        #[cfg(feature = "validators")]
        write_validators,
    ]
    .par_iter()
    .map(|task| task(&gen_context, out_dir))
    .collect::<Result<Vec<()>, _>>()?;

    Ok(())
}

pub(crate) fn write_common(
    _gen_context: &GenContext,
    out_base_dir: &Path,
) -> Result<(), BuildErrorReport> {
    let out_dir = out_base_dir.join("common");
    fs::create_dir_all(&out_dir).map_err(BuildError::from)?;

    fs::write(
        out_dir.join("simple_type.rs"),
        include_bytes!("includes/simple_type.rs"),
    )
    .map_err(BuildError::from)?;

    fs::write(
        out_dir.join("opc_content_types.rs"),
        include_bytes!("includes/packages/opc_content_types.rs"),
    )
    .map_err(BuildError::from)?;

    fs::write(
        out_dir.join("opc_relationships.rs"),
        include_bytes!("includes/packages/opc_relationships.rs"),
    )
    .map_err(BuildError::from)?;

    fs::write(
        out_dir.join("opc_core_properties.rs"),
        include_bytes!("includes/packages/opc_core_properties.rs"),
    )
    .map_err(BuildError::from)?;

    let mut mod_rs_content = quote! {
        pub mod simple_type;
        pub mod opc_content_types;
        pub mod opc_core_properties;
        pub mod opc_relationships;
    }
    .to_string();
    mod_rs_content.push_str(include_str!("includes/common.rs"));

    fs::write(out_dir.join("mod.rs"), mod_rs_content).map_err(BuildError::from)?;

    Ok(())
}

pub(crate) fn write_schemas(
    gen_context: &GenContext,
    out_base_dir: &Path,
) -> Result<(), BuildErrorReport> {
    let out_dir = out_base_dir.join("schemas");
    fs::create_dir_all(&out_dir).map_err(BuildError::from)?;

    let mod_rs_lines = gen_context
        .schemas
        .par_iter()
        .map(|schema| {
            return generate_pub_item_mod(
                &out_dir,
                &schema.module_name,
                &gen_open_xml_schemas(schema, gen_context)?,
            );
        })
        .collect::<Result<Vec<_>, _>>()?;

    fs::write(out_dir.join("mod.rs"), mod_rs_lines.join("\n")).map_err(BuildError::from)?;

    Ok(())
}

pub(crate) fn write_deserializers(
    gen_context: &GenContext,
    out_base_dir: &Path,
) -> Result<(), BuildErrorReport> {
    let out_dir = &out_base_dir.join("deserializers");
    fs::create_dir_all(out_dir).map_err(BuildError::from)?;

    let mod_rs_lines = gen_context
        .schemas
        .iter()
        .map(|schema| {
            return generate_pub_item_mod(
                out_dir,
                &schema.module_name,
                &gen_deserializers(schema, gen_context)?,
            );
        })
        .collect::<Result<Vec<_>, _>>()?;

    fs::write(out_dir.join("mod.rs"), mod_rs_lines.join("\n")).map_err(BuildError::from)?;

    Ok(())
}

pub(crate) fn write_serializers(
    gen_context: &GenContext,
    out_base_dir: &Path,
) -> Result<(), BuildErrorReport> {
    let out_dir = &out_base_dir.join("serializers");
    fs::create_dir_all(out_dir).map_err(BuildError::from)?;

    let mod_rs_lines = gen_context
        .schemas
        .iter()
        .map(|schema| {
            return generate_pub_item_mod(
                out_dir,
                &schema.module_name,
                &gen_serializer(schema, gen_context)?,
            );
        })
        .collect::<Result<Vec<_>, _>>()?;

    fs::write(out_dir.join("mod.rs"), mod_rs_lines.join("\n")).map_err(BuildError::from)?;

    Ok(())
}

#[cfg(feature = "parts")]
pub(crate) fn write_parts(
    gen_context: &GenContext,
    out_base_dir: &Path,
) -> Result<(), BuildErrorReport> {
    use crate::generator::open_xml_part::gen_open_xml_parts;

    let out_dir = &out_base_dir.join("parts");
    fs::create_dir_all(out_dir).map_err(BuildError::from)?;

    let mod_rs_lines = gen_context
        .parts
        .par_iter()
        .map(|part| {
            return generate_pub_item_mod(
                out_dir,
                &part.module_name,
                &gen_open_xml_parts(part, gen_context)?,
            );
        })
        .collect::<Result<Vec<_>, _>>()?;

    fs::write(out_dir.join("mod.rs"), mod_rs_lines.join("\n")).map_err(BuildError::from)?;

    Ok(())
}

#[cfg(feature = "validators")]
pub(crate) fn write_validators(
    gen_context: &GenContext,
    out_base_dir: &Path,
) -> Result<(), BuildErrorReport> {
    use crate::generator::validator::gen_validators;

    let out_dir = &out_base_dir.join("validators");
    fs::create_dir_all(out_dir).map_err(BuildError::from)?;

    let mod_rs_lines = gen_context
        .schemas
        .par_iter()
        .map(|part| {
            return generate_pub_item_mod(
                out_dir,
                &part.module_name,
                &gen_validators(part, gen_context)?,
            );
        })
        .collect::<Result<Vec<_>, _>>()?;

    fs::write(out_dir.join("mod.rs"), mod_rs_lines.join("\n")).map_err(BuildError::from)?;

    Ok(())
}

pub(crate) fn generate_pub_item_mod(
    directory: &Path,
    module_name: &str,
    module_content: &str,
) -> Result<String, BuildErrorReport> {
    fs::write(
        directory.join(module_name).with_extension("rs"),
        module_content,
    )
    .map_err(BuildError::from)?;

    let mod_ident: Ident = parse_str(module_name).map_err(BuildError::from)?;
    let mod_item: ItemMod = parse_quote! {
        pub mod #mod_ident;
    };

    return Ok(mod_item.to_token_stream().to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use rootcause::prelude::*;

    #[test]
    fn test_gen() -> Result<(), Report> {
        let out_dir = tempfile::tempdir()?;
        generate(out_dir.path().join("./test_gen/")).unwrap();

        Ok(())
    }
}
