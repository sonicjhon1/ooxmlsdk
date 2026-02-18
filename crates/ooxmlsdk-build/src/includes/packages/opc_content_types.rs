use super::super::common::*;
use quick_xml::events::BytesStart;
use rootcause::option_ext::OptionExt;

#[derive(Clone, Debug, Default)]
pub struct Types {
    pub xmlns: XmlNamespace,
    pub children: Vec<TypesChildChoice>,
}

#[derive(Clone, Debug, Default)]
pub enum TypesChildChoice {
    Default(Box<Default>),
    Override(Box<Override>),
    #[default]
    None,
}

impl Taggable for Types {
    const NAME: &str = "Types";
}

impl Deserializeable for Types {
    fn deserialize_attributes<'de>(
        mut self,
        xml_reader: &impl XmlReader<'de>,
        xml_event: BytesStart<'de>,
    ) -> Result<Self, SdkErrorReport> {
        let mut xmlns = XmlNamespace::default();

        for attr in xml_event.attributes().with_checks(false) {
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
        let mut children = vec![];

        loop {
            match BytesEvent::expect(xml_reader)? {
                BytesEvent::BytesStart(bytes_start, is_empty)
                    if Default::matched_bytes_start(&bytes_start) =>
                {
                    let mut child =
                        Default::default().deserialize_attributes(xml_reader, bytes_start)?;

                    if !is_empty {
                        child = child.deserialize_children(xml_reader)?;
                    }

                    children.push(TypesChildChoice::Default(std::boxed::Box::new(child)))
                }
                BytesEvent::BytesStart(bytes_start, is_empty)
                    if Override::matched_bytes_start(&bytes_start) =>
                {
                    let mut child =
                        Override::default().deserialize_attributes(xml_reader, bytes_start)?;

                    if !is_empty {
                        child = child.deserialize_children(xml_reader)?;
                    }

                    children.push(TypesChildChoice::Override(std::boxed::Box::new(child)))
                }
                BytesEvent::End(bytes_end) if Self::matched_bytes_end(&bytes_end) => break,
                other => {
                    tracing::warn!("Unhandled event: ({other:?}) from schema: (Types)");
                }
            }
        }

        self.children = children;

        Ok(self)
    }
}

impl Serializeable for Types {
    fn xml_tag_attributes(&self, with_xmlns: bool) -> Option<String> {
        return Some(self.xmlns.serialize_attributes(with_xmlns));
    }

    fn xml_inner(&self, with_xmlns: bool) -> Option<String> {
        let mut xml = String::with_capacity(32);

        for child in &self.children {
            match child {
                TypesChildChoice::Default(child) => {
                    xml.push_str(&child.to_xml_string(false, with_xmlns))
                }
                TypesChildChoice::Override(child) => {
                    xml.push_str(&child.to_xml_string(false, with_xmlns))
                }
                TypesChildChoice::None => (),
            }
        }

        return Some(xml);
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Default {
    pub extension: String,
    pub content_type: String,
}

impl Taggable for Default {
    const NAME: &str = "Default";
}

impl Deserializeable for Default {
    fn deserialize_attributes<'de>(
        mut self,
        xml_reader: &impl XmlReader<'de>,
        xml_event: BytesStart<'de>,
    ) -> Result<Self, SdkErrorReport> {
        let mut extension = None;
        let mut content_type = None;

        for attr in xml_event.attributes().with_checks(false) {
            let attr = attr.map_err(SdkError::from)?;

            match attr.key.0 {
                b"Extension" => {
                    extension = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned(),
                    );
                }
                b"ContentType" => {
                    content_type = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned(),
                    );
                }
                _ => {}
            }
        }

        self.extension =
            extension.context_with(|| SdkError::CommonError("extension".to_string()))?;
        self.content_type =
            content_type.context_with(|| SdkError::CommonError("content_type".to_string()))?;

        Ok(self)
    }
}

impl Serializeable for Default {
    fn xml_tag_attributes(&self, _with_xmlns: bool) -> Option<String> {
        let mut attributes =
            String::with_capacity(const { "Extension".len() + "ContentType".len() + 32 });

        attributes.push_str(&as_xml_attribute("Extension", &self.extension));
        attributes.push_str(&as_xml_attribute("ContentType", &self.content_type));

        return Some(attributes);
    }

    fn xml_inner(&self, _with_xmlns: bool) -> Option<String> { None }
}

#[derive(Clone, Debug, Default)]
pub struct Override {
    pub content_type: String,
    pub part_name: String,
}

impl Taggable for Override {
    const NAME: &str = "Override";
}

impl Deserializeable for Override {
    fn deserialize_attributes<'de>(
        mut self,
        xml_reader: &impl XmlReader<'de>,
        xml_event: BytesStart<'de>,
    ) -> Result<Self, SdkErrorReport> {
        let mut content_type = None;
        let mut part_name = None;

        for attr in xml_event.attributes().with_checks(false) {
            let attr = attr.map_err(SdkError::from)?;

            match attr.key.as_ref() {
                b"ContentType" => {
                    content_type = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned(),
                    );
                }
                b"PartName" => {
                    part_name = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned(),
                    );
                }
                _ => {}
            }
        }

        self.content_type =
            content_type.context_with(|| SdkError::CommonError("content_type".to_string()))?;

        self.part_name =
            part_name.context_with(|| SdkError::CommonError("part_name".to_string()))?;

        Ok(self)
    }
}

impl Serializeable for Override {
    fn xml_tag_attributes(&self, _with_xmlns: bool) -> Option<String> {
        let mut attributes =
            String::with_capacity(const { "Extension".len() + "PartName".len() + 32 });

        attributes.push_str(&as_xml_attribute("ContentType", &self.content_type));
        attributes.push_str(&as_xml_attribute("PartName", &self.part_name));

        return Some(attributes);
    }

    fn xml_inner(&self, _with_xmlns: bool) -> Option<String> { None }
}
