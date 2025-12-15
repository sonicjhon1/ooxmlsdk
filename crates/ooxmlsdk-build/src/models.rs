use crate::utils::escape_snake_case;
use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use syn::{Ident, parse_str};

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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchema {
    pub target_namespace: String,
    pub types: Vec<OpenXmlSchemaType>,
    pub enums: Vec<OpenXmlSchemaEnum>,
    #[serde(skip)]
    pub module_name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaType {
    pub name: String,
    pub class_name: String,
    pub summary: String,
    pub version: String,
    pub part: String,
    pub composite_type: String,
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
    pub fn is_one_sequence_flatten(&self) -> bool {
        (self.composite_type == "OneSequence" || self.particle.kind == "Sequence")
            && self
                .particle
                .items
                .iter()
                .all(|p| p.kind.is_empty() && p.items.is_empty())
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
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeAttribute {
    pub q_name: String,
    pub property_name: String,
    pub r#type: String,
    pub property_comments: String,
    pub version: String,
    pub validators: Vec<OpenXmlSchemaTypeAttributeValidator>,
}

impl OpenXmlSchemaTypeAttribute {
    pub fn as_name_ident(&self) -> Ident {
        let attr_value_ident_raw = if self.property_name.is_empty() {
            &self.q_name
        } else {
            &self.property_name
        };

        return parse_str(&escape_snake_case(attr_value_ident_raw)).unwrap();
    }

    pub fn as_name_str(&self) -> &str { return self.q_name.trim_prefix(":"); }

    pub fn split_type_trimmed(&self) -> (&str, &str) {
        self.r#type
            .rsplit_once('.')
            .map(|(f, l)| (f.trim_start_matches('<'), l.trim_end_matches('>')))
            .unwrap()
    }

    pub fn is_validator_required(&self) -> bool {
        return self
            .validators
            .iter()
            .any(|validator| validator.name == "RequiredValidator");
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
        return parse_str(&self.split_name().1.to_upper_camel_case()).unwrap();
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
        return parse_str(&escape_snake_case(self.as_property_name_str())).unwrap();
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaTypeParticle {
    pub kind: String,
    pub name: String,
    pub occurs: Vec<OpenXmlSchemaTypeParticleOccur>,
    pub items: Vec<OpenXmlSchemaTypeParticle>,
    pub initial_version: String,
    pub require_filter: bool,
    pub namespace: String,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct OpenXmlSchemaEnumFacet {
    pub name: String,
    pub value: String,
    pub version: String,
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
