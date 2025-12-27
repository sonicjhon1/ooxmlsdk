use super::super::common::*;
use quick_xml::events::BytesStart;

#[derive(Clone, Debug, Default)]
pub struct CoreProperties {
    pub xmlns: Option<String>,
    pub xmlns_map: std::collections::HashMap<String, String>,
    pub mc_ignorable: Option<String>,
    pub category: Option<String>,
    pub content_status: Option<String>,
    pub created: Option<String>,
    pub creator: Option<String>,
    pub description: Option<String>,
    pub identifier: Option<String>,
    pub keywords: Option<String>,
    pub language: Option<String>,
    pub last_modified_by: Option<String>,
    pub last_printed: Option<String>,
    pub modified: Option<String>,
    pub revision: Option<String>,
    pub subject: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
}

impl Deserializeable for CoreProperties {
    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport> {
        let (e, empty_tag) = expect_event_start(
            xml_reader,
            xml_event,
            b"cp:coreProperties",
            b"coreProperties",
        )?;

        let mut xmlns = None;

        let mut xmlns_map = std::collections::HashMap::<String, String>::new();

        let mut mc_ignorable = None;

        let mut category: Option<String> = None;

        let mut content_status: Option<String> = None;

        let mut created: Option<String> = None;

        let mut creator: Option<String> = None;

        let mut description: Option<String> = None;

        let mut identifier: Option<String> = None;

        let mut keywords: Option<String> = None;

        let mut language: Option<String> = None;

        let mut last_modified_by: Option<String> = None;

        let mut last_printed: Option<String> = None;

        let mut modified: Option<String> = None;

        let mut revision: Option<String> = None;

        let mut subject: Option<String> = None;

        let mut title: Option<String> = None;

        let mut version: Option<String> = None;

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
                match xml_reader.next()? {
                    quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e) => {
                        match e.name().as_ref() {
                            b"cp:category" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    category = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"cp:contentStatus" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    content_status =
                                        Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dcterms:created" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    created = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dc:creator" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    creator = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dc:description" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    description =
                                        Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dc:identifier" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    identifier =
                                        Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"cp:keywords" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    keywords = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dc:language" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    language = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"cp:lastModifiedBy" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    last_modified_by =
                                        Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"cp:lastPrinted" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    last_printed =
                                        Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dcterms:modified" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    modified = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"cp:revision" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    revision = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dc:subject" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    subject = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"dc:title" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    title = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            b"cp:version" => {
                                if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                                    version = Some(t.decode().map_err(SdkError::from)?.to_string())
                                }

                                xml_reader.next()?;
                            }
                            _ => Err(SdkError::CommonError("coreProperties".to_string()))?,
                        }
                    }
                    quick_xml::events::Event::End(e) => match e.name().as_ref() {
                        b"cp:coreProperties" | b"coreProperties" => {
                            break;
                        }
                        _ => (),
                    },
                    quick_xml::events::Event::Eof => Err(SdkError::UnknownError)?,
                    _ => (),
                }
            }
        }

        Ok(Self {
            xmlns,
            xmlns_map,
            mc_ignorable,
            category,
            content_status,
            created,
            creator,
            description,
            identifier,
            keywords,
            language,
            last_modified_by,
            last_printed,
            modified,
            revision,
            subject,
            title,
            version,
        })
    }
}

impl Serializeable for CoreProperties {
    const PREFIXED_NAME: &str = "cp:coreProperties";

    const NAME: &str = "coreProperties";

    fn xml_tag_attributes(&self, with_xmlns: bool) -> Option<String> {
        let mut attributes = String::with_capacity(
            const { "xmlns".len() + "xmlns:".len() + "mc:Ignorable".len() + 32 },
        );

        if with_xmlns && let Some(xmlns) = &self.xmlns {
            attributes.push_str(&as_xml_attribute("xmlns", xmlns));
        }

        for (key, value) in &self.xmlns_map {
            attributes.push_str(&as_xml_attribute(&format!("xmlns:{key}"), value));
        }

        if let Some(mc_ignorable) = &self.mc_ignorable {
            attributes.push_str(&as_xml_attribute("mc:Ignorable", mc_ignorable));
        }

        return Some(attributes);
    }

    fn xml_inner(&self, _with_xmlns: bool) -> Option<String> {
        let mut xml = String::with_capacity(512);

        if let Some(category) = &self.category {
            xml.push_str("<cp:category>");
            xml.push_str(&quick_xml::escape::escape(category));
            xml.push_str("</cp:category>");
        }

        if let Some(content_status) = &self.content_status {
            xml.push_str("<cp:contentStatus>");
            xml.push_str(&quick_xml::escape::escape(content_status));
            xml.push_str("</cp:contentStatus>");
        }

        if let Some(created) = &self.created {
            xml.push_str(r#"<dcterms:created xsi:type="dcterms:W3CDTF">"#);
            xml.push_str(&quick_xml::escape::escape(created));
            xml.push_str("</dcterms:created>");
        }

        if let Some(creator) = &self.creator {
            xml.push_str("<dc:creator>");
            xml.push_str(&quick_xml::escape::escape(creator));
            xml.push_str("</dc:creator>");
        }

        if let Some(description) = &self.description {
            xml.push_str("<dc:description>");
            xml.push_str(&quick_xml::escape::escape(description));
            xml.push_str("</dc:description>");
        }

        if let Some(identifier) = &self.identifier {
            xml.push_str("<dc:identifier>");
            xml.push_str(&quick_xml::escape::escape(identifier));
            xml.push_str("</dc:identifier>");
        }

        if let Some(keywords) = &self.keywords {
            xml.push_str("<cp:keywords>");
            xml.push_str(&quick_xml::escape::escape(keywords));
            xml.push_str("</cp:keywords>");
        }

        if let Some(language) = &self.language {
            xml.push_str("<dc:language>");
            xml.push_str(&quick_xml::escape::escape(language));
            xml.push_str("</dc:language>");
        }

        if let Some(last_modified_by) = &self.last_modified_by {
            xml.push_str("<cp:lastModifiedBy>");
            xml.push_str(&quick_xml::escape::escape(last_modified_by));
            xml.push_str("</cp:lastModifiedBy>");
        }

        if let Some(last_printed) = &self.last_printed {
            xml.push_str("<cp:lastPrinted>");
            xml.push_str(&quick_xml::escape::escape(last_printed));
            xml.push_str("</cp:lastPrinted>");
        }

        if let Some(modified) = &self.modified {
            xml.push_str(r#"<dcterms:modified xsi:type="dcterms:W3CDTF">"#);
            xml.push_str(&quick_xml::escape::escape(modified));
            xml.push_str("</dcterms:modified>");
        }

        if let Some(revision) = &self.revision {
            xml.push_str("<cp:revision>");
            xml.push_str(&quick_xml::escape::escape(revision));
            xml.push_str("</cp:revision>");
        }

        if let Some(subject) = &self.subject {
            xml.push_str("<dc:subject>");
            xml.push_str(&quick_xml::escape::escape(subject));
            xml.push_str("</dc:subject>");
        }

        if let Some(title) = &self.title {
            xml.push_str("<dc:title>");
            xml.push_str(&quick_xml::escape::escape(title));
            xml.push_str("</dc:title>");
        }

        if let Some(version) = &self.version {
            xml.push_str("<cp:version>");
            xml.push_str(&quick_xml::escape::escape(version));
            xml.push_str("</cp:version>");
        }

        return Some(xml);
    }
}
