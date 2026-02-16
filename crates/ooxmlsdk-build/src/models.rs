use crate::{
    error::BuildErrorReport,
    generator::context::{GenContext, check_office_version},
    utils::{escape_snake_case, escape_upper_camel_case},
};
use heck::ToUpperCamelCase;
use quote::format_ident;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use syn::{Ident, Type, parse_quote};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlPart {
    pub root: String,
    pub name: String,
    pub base: String,
    pub content_type: String,
    pub relationship_type: String,
    pub target: String,
    pub root_element: String,
    pub extension: String,
    pub paths: OpenXmlPartPaths,
    pub version: String,
    pub children: Vec<OpenXmlPartChild>,
    #[serde(skip)]
    pub module_name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlPartPaths {
    pub general: String,
    pub word: String,
    pub excel: String,
    pub power_point: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlPartChild {
    pub min_occurs_is_non_zero: bool,
    pub max_occurs_great_than_one: bool,
    pub api_name: String,
    pub name: String,
    pub has_fixed_content: bool,
    pub is_data_part_reference: bool,
    pub is_special_embedded_part: bool,
}

impl OpenXmlPartChild {
    pub fn as_occurrence(&self) -> Occurrence {
        if self.max_occurs_great_than_one {
            Occurrence::Repeated
        } else if self.min_occurs_is_non_zero {
            Occurrence::Required
        } else {
            Occurrence::Optional
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchema {
    pub target_namespace: String,
    pub types: Vec<OpenXmlSchemaType>,
    pub enums: Vec<OpenXmlSchemaEnum>,
    #[serde(skip)]
    pub module_name: String,
}

impl OpenXmlSchema {
    pub fn module_name_ident(&self) -> Ident { return format_ident!("{}", self.module_name); }

    pub fn struct_type(&self, schema_type: &OpenXmlSchemaType) -> Type {
        let module_name = self.module_name_ident();
        let ident = schema_type.class_name_ident();

        return parse_quote!(crate::schemas::#module_name::#ident);
    }

    pub fn enum_type(&self, schema_enum: &OpenXmlSchemaEnum) -> Type {
        let module_name = self.module_name_ident();
        let ident = schema_enum.name_ident();

        return parse_quote!(crate::schemas::#module_name::#ident);
    }

    pub fn enum_child_choice_type(&self, schema_type: &OpenXmlSchemaType) -> Type {
        let module_name = self.module_name_ident();
        let ident = schema_type.child_choice_class_name_ident();

        return parse_quote!(crate::schemas::#module_name::#ident);
    }

    pub fn needs_xmlns(&self, schema_type: &OpenXmlSchemaType) -> bool {
        !schema_type.part.is_empty()
            || schema_type.base_class == "OpenXmlPartRootElement"
            || ((schema_type.base_class == "OpenXmlCompositeElement"
                || schema_type.base_class == "CustomXmlElement"
                || schema_type.base_class == "OpenXmlPartRootElement"
                || schema_type.base_class == "SdtElement")
                && (self.target_namespace
                    == "http://schemas.openxmlformats.org/drawingml/2006/main"
                    || self.target_namespace
                        == "http://schemas.openxmlformats.org/drawingml/2006/picture"))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaType {
    pub name: String,
    pub class_name: String,
    pub summary: String,
    pub version: String,
    pub part: String,
    pub composite_type: Option<CompositeType>,
    pub base_class: String,
    pub is_leaf_text: bool,
    pub is_leaf_element: bool,
    pub is_derived: bool,
    pub is_abstract: bool,
    pub attributes: Vec<OpenXmlSchemaTypeAttribute>,
    pub children: Vec<OpenXmlSchemaTypeChild>,
    pub particle: OpenXmlSchemaTypeParticle,
    #[serde(skip)]
    pub module_name: String,
}

impl OpenXmlSchemaType {
    #[inline(always)]
    pub fn is_one_sequence_flatten(&self) -> bool {
        (self.composite_type == Some(CompositeType::OneSequence)
            || self.particle.kind == Some(ParticleKind::Sequence))
            && self
                .particle
                .items
                .par_iter()
                .all(|p| p.kind.is_none() && p.items.is_empty())
    }

    #[inline(always)]
    pub fn split_name(&self) -> (&str, &str) { return self.name.split_once('/').unwrap() }

    #[inline(always)]
    pub fn split_last_name(&self) -> (&str, &str) {
        return self.split_name().1.split_once(':').unwrap();
    }

    #[inline(always)]
    pub fn child_map(&self) -> HashMap<&str, &OpenXmlSchemaTypeChild> {
        let mut child_map = HashMap::with_capacity(self.children.len());
        for child in &self.children {
            child_map.insert(child.name.as_str(), child);
        }

        return child_map;
    }

    #[inline(always)]
    pub fn child_iter(&self) -> impl Iterator<Item = (&str, &OpenXmlSchemaTypeChild)> {
        return self
            .children
            .iter()
            .map(|child| (child.name.as_str(), child));
    }

    #[inline(always)]
    pub fn class_name_ident(&self) -> Ident {
        format_ident!("{}", self.class_name.to_upper_camel_case())
    }

    #[inline(always)]
    pub fn child_choice_class_name_ident(&self) -> Ident {
        format_ident!("{}ChildChoice", self.class_name.to_upper_camel_case())
    }

    #[inline(always)]
    pub fn module_name_ident(&self) -> Ident { return format_ident!("{}", self.module_name) }

    #[inline(always)]
    pub fn r#type(&self, is_same_module: bool) -> Type {
        let ident = self.class_name_ident();

        if is_same_module {
            return parse_quote!(#ident);
        } else {
            let module_name = self.module_name_ident();

            return parse_quote!(crate::schemas::#module_name::#ident);
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub enum CompositeType {
    OneAll,
    OneChoice,
    OneSequence,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum OpenXmlSchemaTypeAttributeType {
    ListValue { r#type: String },
    EnumValue { namespace: String, r#type: String },
    SimpleType { r#type: String },
}

impl TryFrom<String> for OpenXmlSchemaTypeAttributeType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Some(r#type) = value.strip_circumfix("ListValue<", ">") {
            Ok(Self::ListValue {
                r#type: r#type.to_owned(),
            })
        } else if let Some(namespace_and_type) = value.strip_circumfix("EnumValue<", ">") {
            let (namespace, r#type) = namespace_and_type
                .rsplit_once('.')
                .ok_or_else(|| format!("EnumValue ({value}) doesn't contain '.'"))?;

            Ok(Self::EnumValue {
                namespace: namespace.to_owned(),
                r#type: r#type.to_owned(),
            })
        } else {
            Ok(Self::SimpleType {
                r#type: value.clone(),
            })
        }
    }
}

impl From<OpenXmlSchemaTypeAttributeType> for String {
    fn from(value: OpenXmlSchemaTypeAttributeType) -> Self {
        match value {
            OpenXmlSchemaTypeAttributeType::ListValue { r#type } => format!("ListValue<{type}>"),
            OpenXmlSchemaTypeAttributeType::EnumValue { namespace, r#type } => {
                format!("EnumValue<{namespace}.{type}>")
            }
            OpenXmlSchemaTypeAttributeType::SimpleType { r#type } => r#type.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeAttribute {
    pub q_name: String,
    pub property_name: String,
    pub r#type: Option<OpenXmlSchemaTypeAttributeType>,
    pub property_comments: String,
    pub version: String,
    pub validators: Vec<OpenXmlSchemaTypeAttributeValidator>,
}

impl OpenXmlSchemaTypeAttribute {
    #[inline(always)]
    pub fn as_name_ident(&self) -> Ident {
        let attr_value_ident_raw = if self.property_name.is_empty() {
            &self.q_name
        } else {
            &self.property_name
        };

        return format_ident!("{}", escape_snake_case(attr_value_ident_raw));
    }

    #[inline(always)]
    pub fn as_name_str(&self) -> &str { return self.q_name.trim_prefix(":"); }

    #[inline(always)]
    pub fn is_validator_required(&self) -> bool {
        return self
            .validators
            .iter()
            .any(|validator| validator.name == "RequiredValidator");
    }

    pub fn r#type(&self, gen_context: &GenContext) -> Result<Type, BuildErrorReport> {
        match self.r#type.as_ref() {
            Some(OpenXmlSchemaTypeAttributeType::ListValue { .. }) => {
                return Ok(parse_quote!(String));
            }
            Some(OpenXmlSchemaTypeAttributeType::EnumValue { namespace, r#type }) => {
                let (enum_schema, enum_schema_enum) = gen_context
                    .typed_namespaces
                    .iter()
                    .find_map(|typed_namespace| {
                        if typed_namespace.namespace != *namespace {
                            return None;
                        };

                        let schema = gen_context
                            .prefix_schema_map
                            .get(typed_namespace.prefix.as_str())?;

                        return schema
                            .enums
                            .iter()
                            .find(|schema_enum| schema_enum.name == *r#type)
                            .map(|schema_enum| (schema, schema_enum));
                    })
                    .unwrap();

                return Ok(enum_schema.enum_type(enum_schema_enum));
            }
            Some(OpenXmlSchemaTypeAttributeType::SimpleType { r#type }) => {
                let ident = format_ident!("{type}");

                return Ok(parse_quote!(crate::common::simple_type::#ident));
            }
            None => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeAttributeValidator {
    pub name: String,
    pub is_list: bool,
    pub r#type: String,
    pub union_id: u64,
    pub is_initial_version: bool,
    pub arguments: Vec<OpenXmlSchemaTypeAttributeValidatorArgument>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeAttributeValidatorArgument {
    pub name: String,
    pub r#type: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeChild {
    pub name: String,
    pub property_name: String,
    pub property_comments: String,
}

impl OpenXmlSchemaTypeChild {
    #[inline(always)]
    pub fn split_name(&self) -> (&str, &str) { return self.name.split_once('/').unwrap() }

    #[inline(always)]
    pub fn split_last_name(&self) -> (&str, &str) {
        return self.split_name().1.split_once(':').unwrap();
    }

    #[inline(always)]
    pub fn as_last_name_ident(&self) -> Ident {
        return format_ident!("{}", self.split_name().1.to_upper_camel_case());
    }

    #[inline(always)]
    pub fn as_property_name_str(&self) -> &str {
        return if self.property_name.is_empty() {
            self.split_name().1
        } else {
            self.property_name.as_str()
        };
    }

    #[inline(always)]
    pub fn as_property_name_ident(&self) -> Ident {
        return format_ident!("{}", escape_snake_case(self.as_property_name_str()));
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeParticle {
    pub kind: Option<ParticleKind>,
    pub name: String,
    pub occurs: Vec<OpenXmlSchemaTypeParticleOccur>,
    pub items: Vec<OpenXmlSchemaTypeParticle>,
    pub initial_version: String,
    pub require_filter: bool,
    pub namespace: String,
}

impl OpenXmlSchemaTypeParticle {
    #[inline(always)]
    pub fn check_particle_version(&mut self) {
        self.items
            .retain(|x| check_office_version(&x.initial_version));

        for item in self.items.iter_mut() {
            if item.kind.is_none() {
                item.check_particle_version();
            }
        }
    }

    pub fn as_occurrence(&self) -> Occurrence {
        if self.occurs.is_empty() {
            Occurrence::Required
        } else if self.occurs[0].min == 0 && self.occurs[0].max == 1 {
            Occurrence::Optional
        } else {
            Occurrence::Repeated
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub enum ParticleKind {
    All,
    Any,
    Choice,
    Group,
    Sequence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Occurrence {
    Required,
    Optional,
    Repeated,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeParticleOccur {
    pub max: u64,
    pub min: u64,
    pub include_version: bool,
    pub version: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaEnum {
    pub name: String,
    pub r#type: String,
    pub facets: Vec<OpenXmlSchemaEnumFacet>,
    pub version: String,
    #[serde(skip)]
    pub module_name: String,
}

impl OpenXmlSchemaEnum {
    pub fn module_name_ident(&self) -> Ident { return format_ident!("{}", self.module_name) }

    pub fn name_ident(&self) -> Ident {
        return format_ident!("{}", self.name.to_upper_camel_case());
    }

    #[inline(always)]
    pub fn r#type(&self, is_same_module: bool) -> Type {
        let ident = self.name_ident();

        if is_same_module {
            return parse_quote!(#ident);
        } else {
            let module_name = self.module_name_ident();

            return parse_quote!(crate::schemas::#module_name::#ident);
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaEnumFacet {
    pub name: String,
    pub value: String,
    pub version: String,
}

impl OpenXmlSchemaEnumFacet {
    #[inline(always)]
    pub fn as_variant(&self) -> &str {
        return if self.name.is_empty() {
            &self.value
        } else {
            &self.name
        };
    }

    #[inline(always)]
    pub fn as_variant_ident(&self) -> Ident {
        return format_ident!("{}", escape_upper_camel_case(self.as_variant()));
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlNamespace {
    pub prefix: String,
    pub uri: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct TypedNamespace {
    pub prefix: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct TypedSchema {
    pub name: String,
    pub class_name: String,
    pub part_class_name: String,
}
