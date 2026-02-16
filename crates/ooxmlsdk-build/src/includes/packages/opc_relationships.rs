use super::super::common::*;
use quick_xml::events::BytesStart;
use rootcause::{option_ext::OptionExt, prelude::ResultExt};

#[derive(Clone, Debug, Default)]
pub struct Relationships {
    pub xmlns: XmlNamespace,
    pub relationship: Vec<Relationship>,
}

impl Taggable for Relationships {
    const NAME: &str = "Relationships";
}

impl Deserializeable for Relationships {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, empty_tag) = expect_event_start::<Self>(xml_reader, xml_event)?;

        let mut xmlns = XmlNamespace::default();

        let mut relationship = vec![];

        for attr in e.attributes() {
            let attr = attr.map_err(SdkError::from)?;
            let _ = xmlns.deserialize_attributes(xml_reader, &attr)?;
        }

        if !empty_tag {
            loop {
                let mut e_opt: Option<BytesStart<'_>> = None;
                let mut e_empty = false;

                match xml_reader.next()? {
                    quick_xml::events::Event::Start(e) => {
                        e_opt = Some(e);
                    }
                    quick_xml::events::Event::Empty(e) => {
                        e_empty = true;
                        e_opt = Some(e);
                    }
                    quick_xml::events::Event::End(e) if Self::matched_name(e.name().0) => {
                        break;
                    }
                    quick_xml::events::Event::Eof => Err(SdkError::UnknownError)?,
                    _ => (),
                }

                if let Some(e) = e_opt {
                    if Relationship::matched_name(e.name().0) {
                        relationship.push(Relationship::deserialize_inner(
                            xml_reader,
                            Some((e, e_empty)),
                        )?);
                    } else {
                        return Err(SdkError::CommonError("Types".to_string()))
                            .attach(String::from_utf8_lossy(e.name().into_inner()).to_string())?;
                    }
                }
            }
        }

        Ok(Self {
            xmlns,
            relationship,
        })
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
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, _) = expect_event_start::<Self>(xml_reader, xml_event)?;

        let mut target_mode = None;

        let mut target = None;

        let mut r#type = None;

        let mut id = None;

        for attr in e.attributes().with_checks(false) {
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

        let target = target.context_with(|| SdkError::CommonError("target".to_string()))?;

        let r#type = r#type.context_with(|| SdkError::CommonError("type".to_string()))?;

        let id = id.context_with(|| SdkError::CommonError("id".to_string()))?;

        Ok(Self {
            target_mode,
            target,
            r#type,
            id,
        })
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
