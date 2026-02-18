use super::super::common::*;
use quick_xml::events::BytesStart;

#[derive(Clone, Debug, Default)]
pub struct CoreProperties {
    pub xmlns: XmlNamespace,
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

impl Taggable for CoreProperties {
    const PREFIXED_NAME: Option<&str> = Some("cp:coreProperties");
    const PREFIX: Option<&str> = Some("cp");
    const NAME: &str = "coreProperties";
}

impl Deserializeable for CoreProperties {
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

        loop {
            match BytesEvent::expect(xml_reader)? {
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"cp:category" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        category = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"cp:contentStatus" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        content_status = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"dcterms:created" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        created = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _) if bytes_start.name().0 == b"dc:creator" => {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        creator = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"dc:description" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        description = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"dc:identifier" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        identifier = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"cp:keywords" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        keywords = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"dc:language" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        language = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"cp:lastModifiedBy" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        last_modified_by = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"cp:lastPrinted" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        last_printed = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"dcterms:modified" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        modified = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _)
                    if bytes_start.name().0 == b"cp:revision" =>
                {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        revision = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _) if bytes_start.name().0 == b"dc:subject" => {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        subject = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _) if bytes_start.name().0 == b"dc:title" => {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        title = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::BytesStart(bytes_start, _) if bytes_start.name().0 == b"cp:version" => {
                    if let quick_xml::events::Event::Text(t) = xml_reader.next()? {
                        version = Some(t.decode().map_err(SdkError::from)?.to_string())
                    }

                    xml_reader.next()?;
                }
                BytesEvent::End(bytes_end) if Self::matched_bytes_end(&bytes_end) => break,
                other => {
                    tracing::warn!("Unhandled event: ({other:?}) from schema: (CoreProperties)");
                }
            }
        }

        self.category = category;
        self.content_status = content_status;
        self.created = created;
        self.creator = creator;
        self.description = description;
        self.identifier = identifier;
        self.keywords = keywords;
        self.language = language;
        self.last_modified_by = last_modified_by;
        self.last_printed = last_printed;
        self.modified = modified;
        self.revision = revision;
        self.subject = subject;
        self.title = title;
        self.version = version;

        Ok(self)
    }
}

impl Serializeable for CoreProperties {
    fn xml_tag_attributes(&self, with_xmlns: bool) -> Option<String> {
        return Some(self.xmlns.serialize_attributes(with_xmlns));
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
