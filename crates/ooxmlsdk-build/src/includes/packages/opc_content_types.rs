use super::super::common::*;
use quick_xml::events::BytesStart;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Types {
    pub xmlns: Option<String>,
    pub xmlns_map: HashMap<String, String>,
    pub mc_ignorable: Option<String>,
    pub children: Vec<TypesChildChoice>,
}

#[derive(Clone, Debug, Default)]
pub enum TypesChildChoice {
    Default(Box<Default>),
    Override(Box<Override>),
    #[default]
    None,
}

impl Deserializeable for Types {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, empty_tag) = expect_event_start(xml_reader, xml_event, b"w:Types", b"Types")?;

        let mut xmlns = None;
        let mut xmlns_map = HashMap::<String, String>::new();
        let mut mc_ignorable = None;

        let mut children = vec![];

        for attr in e.attributes().with_checks(false) {
            let attr = attr.map_err(SdkError::from)?;

            match attr.key.as_ref() {
                b"xmlns" => {
                    xmlns = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned(),
                    );
                }
                b"mc:Ignorable" => {
                    mc_ignorable = Some(
                        attr.decode_and_unescape_value(xml_reader.decoder())
                            .map_err(SdkError::from)?
                            .into_owned(),
                    );
                }
                key => {
                    if key.starts_with(b"xmlns:") {
                        xmlns_map.insert(
                            String::from_utf8_lossy(&key[6..]).to_string(),
                            attr.decode_and_unescape_value(xml_reader.decoder())
                                .map_err(SdkError::from)?
                                .into_owned(),
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
                        b"w:Types" | b"Types" => {
                            break;
                        }
                        _ => (),
                    },
                    quick_xml::events::Event::Eof => Err(SdkError::UnknownError)?,
                    _ => (),
                }

                if let Some(e) = e_opt {
                    match e.name().as_ref() {
                        b"w:Default" | b"Default" => {
                            children.push(TypesChildChoice::Default(std::boxed::Box::new(
                                Default::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                            )));
                        }
                        b"w:Override" | b"Override" => {
                            children.push(TypesChildChoice::Override(std::boxed::Box::new(
                                Override::deserialize_inner(xml_reader, Some((e, e_empty)))?,
                            )));
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
            children,
        })
    }
}

impl Serializeable for Types {
    const PREFIXED_NAME: &str = "Types";

    const NAME: &str = "w:Types";

    fn xml_tag_attributes(&self, needs_xmlns: bool) -> Option<String> {
        let mut attributes = String::with_capacity(
            const { "xmlns".len() + "xmlns:".len() + "mc:ignorable".len() + 32 },
        );

        if needs_xmlns && let Some(xmlns) = &self.xmlns {
            attributes.push_str(&as_xml_attribute("xmlns", xmlns));
        }

        for (key, value) in &self.xmlns_map {
            attributes.push_str(&as_xml_attribute(&format!("xmlns:{key}"), value));
        }

        if let Some(mc_ignorable) = &self.mc_ignorable {
            attributes.push_str(&as_xml_attribute("mc:ignorable", mc_ignorable));
        }

        return Some(attributes);
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

#[derive(Clone, Debug, Default)]
pub struct Default {
    pub extension: String,
    pub content_type: String,
}

impl Deserializeable for Default {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, _) = expect_event_start(xml_reader, xml_event, b"w:Default", b"Default")?;

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

        let extension = extension.ok_or_else(|| SdkError::CommonError("extension".to_string()))?;

        let content_type =
            content_type.ok_or_else(|| SdkError::CommonError("content_type".to_string()))?;

        Ok(Self {
            extension,
            content_type,
        })
    }
}

impl Serializeable for Default {
    const PREFIXED_NAME: &str = "w:Default";

    const NAME: &str = "Default";

    fn xml_tag_attributes(&self, _needs_xmlns: bool) -> Option<String> {
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

impl Deserializeable for Override {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, _) = expect_event_start(xml_reader, xml_event, b"w:Override", b"Override")?;

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
            content_type.ok_or_else(|| SdkError::CommonError("content_type".into()))?;

        let part_name = part_name.ok_or_else(|| SdkError::CommonError("part_name".to_string()))?;

        Ok(Self {
            content_type,
            part_name,
        })
    }
}

impl Serializeable for Override {
    const PREFIXED_NAME: &str = "w:Override";

    const NAME: &str = "Override";

    fn xml_tag_attributes(&self, _needs_xmlns: bool) -> Option<String> {
        let mut attributes =
            String::with_capacity(const { "Extension".len() + "PartName".len() + 32 });

        attributes.push_str(&as_xml_attribute("ContentType", &self.content_type));
        attributes.push_str(&as_xml_attribute("PartName", &self.content_type));

        return Some(attributes);
    }

    fn xml_inner(&self, _with_xmlns: bool) -> Option<String> { None }
}
