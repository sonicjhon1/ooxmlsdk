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
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, empty_tag) = expect_event_start::<Self>(xml_reader, xml_event)?;

        let mut xmlns = XmlNamespace::default();

        let mut children = vec![];

        for attr in e.attributes().with_checks(false) {
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
                    let event_name = e.name().0;

                    if Default::matched_name(event_name) {
                        children.push(TypesChildChoice::Default(std::boxed::Box::new(
                            Default::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                        )))
                    } else if Override::matched_name(event_name) {
                        children.push(TypesChildChoice::Override(std::boxed::Box::new(
                            Override::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                        )));
                    }
                }
            }
        }

        Ok(Self { xmlns, children })
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
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, _) = expect_event_start::<Self>(xml_reader, xml_event)?;

        let mut extension = None;
        let mut content_type = None;

        for attr in e.attributes().with_checks(false) {
            let attr = attr.map_err(SdkError::from)?;

            match attr.key.as_ref() {
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

        let extension =
            extension.context_with(|| SdkError::CommonError("extension".to_string()))?;

        let content_type =
            content_type.context_with(|| SdkError::CommonError("content_type".to_string()))?;

        Ok(Self {
            extension,
            content_type,
        })
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
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, _) = expect_event_start::<Self>(xml_reader, xml_event)?;

        let mut content_type = None;
        let mut part_name = None;

        for attr in e.attributes().with_checks(false) {
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

        let content_type =
            content_type.context_with(|| SdkError::CommonError("content_type".to_string()))?;

        let part_name =
            part_name.context_with(|| SdkError::CommonError("part_name".to_string()))?;

        Ok(Self {
            content_type,
            part_name,
        })
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
