use quick_xml::events::BytesStart;

use super::super::common::*;

#[derive(Clone, Debug, Default)]
pub struct Relationships {
    pub xmlns: Option<String>,
    pub xmlns_map: std::collections::HashMap<String, String>,
    pub mc_ignorable: Option<String>,
    pub relationship: Vec<Relationship>,
}

impl Deserializeable for Relationships {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, empty_tag) =
            expect_event_start(xml_reader, xml_event, b"w:Relationships", b"Relationships")?;

        let mut xmlns = None;

        let mut xmlns_map = std::collections::HashMap::<String, String>::new();

        let mut mc_ignorable = None;

        let mut relationship = vec![];

        for attr in e.attributes() {
            let attr = attr.map_err(SdkError::from)?;
            match attr.key.as_ref() {
                b"xmlns" => {
                    xmlns = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .to_string(),
                    );
                }
                b"mc:Ignorable" => {
                    mc_ignorable = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .to_string(),
                    );
                }
                key => {
                    if key.starts_with(b"xmlns:") {
                        xmlns_map.insert(
                            String::from_utf8_lossy(&key[6..]).to_string(),
                            attr.decode_and_unescape_value(xml_reader.decoder())
                                .map_err(SdkError::from)?
                                .to_string(),
                        );
                    }
                }
            }
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
                    quick_xml::events::Event::End(e) => match e.name().as_ref() {
                        b"w:Relationships" | b"Relationships" => {
                            break;
                        }
                        _ => (),
                    },
                    quick_xml::events::Event::Eof => Err(SdkError::UnknownError)?,
                    _ => (),
                }

                if let Some(e) = e_opt {
                    match e.name().as_ref() {
                        b"w:Relationship" | b"Relationship" => {
                            relationship.push(Relationship::deserialize_inner(
                                xml_reader,
                                Some((e, e_empty)),
                            )?);
                        }

                        _ => Err(SdkError::CommonError("Types".to_string()))?,
                    }
                }
            }
        }

        Ok(Self {
            xmlns,
            xmlns_map,
            mc_ignorable,
            relationship,
        })
    }
}

impl Relationships {
    #[allow(clippy::inherent_to_string)]
    pub fn to_xml(&self) -> Result<String, std::fmt::Error> {
        self.to_string_inner(if let Some(xmlns) = &self.xmlns {
            xmlns != "http://schemas.openxmlformats.org/package/2006/relationships"
        } else {
            true
        })
    }

    pub fn to_string_inner(&self, with_xmlns: bool) -> Result<String, std::fmt::Error> {
        use std::fmt::Write;

        let mut writer = String::new();

        writer.write_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n")?;

        writer.write_char('<')?;

        if with_xmlns {
            writer.write_str("w:Relationships")?;
        } else {
            writer.write_str("Relationships")?;
        }

        if let Some(xmlns) = &self.xmlns {
            writer.write_str(r#" xmlns=""#)?;
            writer.write_str(xmlns)?;
            writer.write_str("\"")?;
        }

        for (k, v) in &self.xmlns_map {
            writer.write_str(" xmlns:")?;
            writer.write_str(k)?;
            writer.write_str("=\"")?;
            writer.write_str(v)?;
            writer.write_str("\"")?;
        }

        if let Some(mc_ignorable) = &self.mc_ignorable {
            writer.write_str(r#" mc:Ignorable=""#)?;
            writer.write_str(mc_ignorable)?;
            writer.write_str("\"")?;
        }

        writer.write_char('>')?;

        for child in &self.relationship {
            let child_str = child.to_string_inner(with_xmlns)?;

            writer.write_str(&child_str)?;
        }

        writer.write_str("</")?;

        if with_xmlns {
            writer.write_str("w:Relationships")?;
        } else {
            writer.write_str("Relationships")?;
        }

        writer.write_char('>')?;

        Ok(writer)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Relationship {
    pub target_mode: Option<TargetMode>,
    pub target: String,
    pub r#type: String,
    pub id: String,
}

impl Deserializeable for Relationship {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, _) = expect_event_start(xml_reader, xml_event, b"w:Relationship", b"Relationship")?;

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

        let target = target.ok_or_else(|| SdkError::CommonError("target".to_string()))?;

        let r#type = r#type.ok_or_else(|| SdkError::CommonError("type".to_string()))?;

        let id = id.ok_or_else(|| SdkError::CommonError("id".to_string()))?;

        Ok(Self {
            target_mode,
            target,
            r#type,
            id,
        })
    }
}

impl Relationship {
    #[allow(clippy::inherent_to_string)]
    pub fn to_xml(&self) -> Result<String, std::fmt::Error> { self.to_string_inner(false) }

    pub fn to_string_inner(&self, with_xmlns: bool) -> Result<String, std::fmt::Error> {
        use std::fmt::Write;

        let mut writer = String::new();

        writer.write_char('<')?;

        if with_xmlns {
            writer.write_str("w:Relationship")?;
        } else {
            writer.write_str("Relationship")?;
        }

        if let Some(target_mode) = &self.target_mode {
            writer.write_char(' ')?;
            writer.write_str("TargetMode")?;
            writer.write_str("=\"")?;
            writer.write_str(&quick_xml::escape::escape(target_mode.to_string()))?;
            writer.write_char('"')?;
        }

        writer.write_char(' ')?;
        writer.write_str("Target")?;
        writer.write_str("=\"")?;
        writer.write_str(&quick_xml::escape::escape(self.target.to_string()))?;
        writer.write_char('"')?;

        writer.write_char(' ')?;
        writer.write_str("Type")?;
        writer.write_str("=\"")?;
        writer.write_str(&quick_xml::escape::escape(self.r#type.to_string()))?;
        writer.write_char('"')?;

        writer.write_char(' ')?;
        writer.write_str("Id")?;
        writer.write_str("=\"")?;
        writer.write_str(&quick_xml::escape::escape(self.id.to_string()))?;
        writer.write_char('"')?;

        writer.write_str("/>")?;

        Ok(writer)
    }
}

#[derive(Clone, Debug, Default)]
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
