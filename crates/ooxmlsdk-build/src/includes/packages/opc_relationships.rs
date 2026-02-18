use super::super::common::*;
use quick_xml::events::BytesStart;
use rootcause::option_ext::OptionExt;

#[derive(Clone, Debug, Default)]
pub struct Relationships {
    pub xmlns: XmlNamespace,
    pub relationship: Vec<Relationship>,
}

impl Taggable for Relationships {
    const NAME: &str = "Relationships";
}

impl Deserializeable for Relationships {
    fn deserialize_attributes<'de>(
        mut self,
        xml_reader: &impl XmlReader<'de>,
        xml_event: BytesStart<'de>,
    ) -> Result<Self, SdkErrorReport> {
        let mut xmlns = XmlNamespace::default();

        for attr in xml_event.attributes() {
            let attr = attr.map_err(SdkError::from)?;
            let _ = xmlns.deserialize_attributes(xml_reader, &attr)?;
        }

        self.xmlns = xmlns;

        Ok(self)
    }

    fn deserialize_children<'de>(
        mut self,
        xml_reader: &mut impl XmlReader<'de>,
    ) -> Result<Self, SdkErrorReport> {
        let mut relationship = vec![];

        loop {
            match BytesEvent::expect(xml_reader)? {
                BytesEvent::BytesStart(bytes_start, is_empty)
                    if Relationship::matched_bytes_start(&bytes_start) =>
                {
                    let mut child =
                        Relationship::default().deserialize_attributes(xml_reader, bytes_start)?;

                    if !is_empty {
                        child = child.deserialize_children(xml_reader)?;
                    }

                    relationship.push(child);
                }
                BytesEvent::End(bytes_end) if Self::matched_bytes_end(&bytes_end) => break,
                other => {
                    tracing::warn!("Unhandled event: ({other:?}) from schema: (Relationships)");
                }
            }
        }

        self.relationship = relationship;

        Ok(self)
    }
}

impl Serializeable for Relationships {
    fn xml_tag_attributes(&self, with_xmlns: bool) -> Option<String> {
        return Some(self.xmlns.serialize_attributes(with_xmlns));
    }

    fn xml_inner(&self, with_xmlns: bool) -> Option<String> {
        let mut xml = String::with_capacity(32);

        for child in &self.relationship {
            xml.push_str(&child.to_xml_string(false, with_xmlns));
        }

        return Some(xml);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Relationship {
    pub target_mode: Option<TargetMode>,
    pub target: String,
    pub r#type: String,
    pub id: String,
}

impl Taggable for Relationship {
    const NAME: &str = "Relationship";
}

impl Deserializeable for Relationship {
    fn deserialize_attributes<'de>(
        mut self,
        xml_reader: &impl XmlReader<'de>,
        xml_event: BytesStart<'de>,
    ) -> Result<Self, SdkErrorReport> {
        let mut target_mode = None;
        let mut target = None;
        let mut r#type = None;
        let mut id = None;

        for attr in xml_event.attributes().with_checks(false) {
            let attr = attr.map_err(SdkError::from)?;

            match attr.key.as_ref() {
                b"TargetMode" => {
                    target_mode = Some(TargetMode::from_str(
                        &attr
                            .decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?,
                    )?);
                }
                b"Target" => {
                    target = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .to_string(),
                    );
                }
                b"Type" => {
                    r#type = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .to_string(),
                    );
                }
                b"Id" => {
                    id = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .to_string(),
                    );
                }
                _ => {}
            }
        }

        self.target_mode = target_mode;
        self.target = target.context_with(|| SdkError::CommonError("target".to_string()))?;
        self.r#type = r#type.context_with(|| SdkError::CommonError("type".to_string()))?;
        self.id = id.context_with(|| SdkError::CommonError("id".to_string()))?;

        Ok(self)
    }
}

impl Serializeable for Relationship {
    fn xml_tag_attributes(&self, _with_xmlns: bool) -> Option<String> {
        let mut attributes = String::with_capacity(
            const { "TargetMode".len() + "Target".len() + "Type".len() + "Id".len() + 32 },
        );

        if let Some(target_mode) = &self.target_mode {
            attributes.push_str(&as_xml_attribute("TargetMode", &target_mode.to_string()));
        }
        attributes.push_str(&as_xml_attribute("Target", &self.target));
        attributes.push_str(&as_xml_attribute("Type", &self.r#type));
        attributes.push_str(&as_xml_attribute("Id", &self.id));

        return Some(attributes);
    }

    fn xml_inner(&self, _with_xmlns: bool) -> Option<String> { None }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum TargetMode {
    #[default]
    External,
    Internal,
}

impl TargetMode {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: impl AsRef<str>) -> Result<Self, SdkErrorReport> {
        match s.as_ref() {
            "External" => Ok(Self::External),
            "Internal" => Ok(Self::Internal),
            _ => Err(SdkError::CommonError(s.as_ref().to_string()))?,
        }
    }
}

impl std::fmt::Display for TargetMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetMode::External => write!(f, "External"),
            TargetMode::Internal => write!(f, "Internal"),
        }
    }
}
