use quick_xml::{
    Decoder, Reader,
    events::{BytesStart, Event, attributes::Attribute},
};
use rootcause::prelude::*;
use std::{collections::BTreeMap, io::BufRead, path::Path};
use thiserror::Error;
use tracing::*;

pub type SdkErrorReport = Report<SdkError>;

#[derive(Error, Debug)]
pub enum SdkError {
    #[error("quick_xml error")]
    QuickXmlError(#[from] quick_xml::Error),
    #[error("quick_xml encoding error")]
    QuickEncodingError(#[from] quick_xml::encoding::EncodingError),
    #[error("quick_xml attr error")]
    AttrError(#[from] quick_xml::events::attributes::AttrError),
    #[error("ParseIntError")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("ParseFloatError")]
    ParseFloatError(#[from] std::num::ParseFloatError),
    #[error("StdFmtError")]
    StdFmtError(#[from] std::fmt::Error),
    #[error("StdIoError")]
    StdIoError(#[from] std::io::Error),
    #[cfg(feature = "parts")]
    #[error("ZipError")]
    ZipError(#[from] zip::result::ZipError),
    #[error("mismatch error (expected {expected:?}, found {found:?})")]
    MismatchError { expected: String, found: String },
    #[error("`{0}` common error")]
    CommonError(String),
    #[error("unknown error")]
    UnknownError,
}

pub trait XmlReader<'de> {
    fn next(&mut self) -> Result<Event<'de>, SdkErrorReport>;
    fn decoder(&self) -> Decoder;
}

pub struct IoReader<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> IoReader<R> {
    #[inline]
    pub fn new(reader: Reader<R>) -> Self {
        Self {
            reader,
            buf: vec![],
        }
    }
}

impl<'de, R: BufRead> XmlReader<'de> for IoReader<R> {
    #[inline]
    fn next(&mut self) -> Result<Event<'de>, SdkErrorReport> {
        self.buf.clear();

        Ok(self
            .reader
            .read_event_into(&mut self.buf)
            .map_err(SdkError::from)?
            .into_owned())
    }

    #[inline]
    fn decoder(&self) -> Decoder { self.reader.decoder() }
}

pub struct SliceReader<'de> {
    reader: Reader<&'de [u8]>,
}

impl<'de> SliceReader<'de> {
    #[inline]
    pub fn new(reader: Reader<&'de [u8]>) -> Self { Self { reader } }
}

impl<'de> XmlReader<'de> for SliceReader<'de> {
    #[inline]
    fn next(&mut self) -> Result<Event<'de>, SdkErrorReport> {
        Ok(self.reader.read_event().map_err(SdkError::from)?)
    }

    #[inline]
    fn decoder(&self) -> Decoder { self.reader.decoder() }
}

pub trait Taggable {
    const PREFIXED_NAME: Option<&str> = None;
    const PREFIX: Option<&str> = None;
    const NAME: &str;

    fn prefixed_name_or_name() -> &'static str { Self::PREFIXED_NAME.unwrap_or(Self::NAME) }

    fn matched_name(bytes: &[u8]) -> bool {
        return bytes == Self::NAME.as_bytes()
            || Some(bytes) == Self::PREFIXED_NAME.map(|s| s.as_bytes());
    }
}

pub trait Deserializeable: Taggable + Sized {
    fn from_str(str: impl AsRef<str>) -> Result<Self, SdkErrorReport> {
        let mut xml_reader = quick_xml::Reader::from_str(str.as_ref());
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_inner(&mut SliceReader::new(xml_reader), None)
    }

    fn from_reader(reader: impl BufRead) -> Result<Self, SdkErrorReport> {
        let mut xml_reader = quick_xml::Reader::from_reader(reader);
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_inner(&mut IoReader::new(xml_reader), None)
    }

    fn from_file(path: impl AsRef<Path>) -> Result<Self, SdkErrorReport> {
        let mut xml_reader = quick_xml::Reader::from_file(path).map_err(SdkError::from)?;
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_inner(&mut IoReader::new(xml_reader), None)
    }

    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport>;
}

pub trait Serializeable: Taggable {
    fn xml_tag_attributes(&self, _with_xmln: bool) -> Option<String> { return None; }

    fn xml_inner(&self, _with_xmlns: bool) -> Option<String> { return None; }

    #[inline]
    fn xml_tag_start(&self, with_xmlns: bool) -> String {
        let mut xml = String::with_capacity(Self::prefixed_name_or_name().len() + 32);

        xml.push('<');

        if with_xmlns {
            xml.push_str(Self::prefixed_name_or_name());
        } else {
            xml.push_str(Self::NAME);
        }

        if let Some(xml_tag_attributes) = self.xml_tag_attributes(with_xmlns) {
            xml.push_str(&xml_tag_attributes);
        }

        xml.push('>');

        return xml;
    }

    #[inline]
    fn xml_tag_end(&self, with_xmlns: bool) -> String {
        let mut xml = String::with_capacity(Self::prefixed_name_or_name().len() + 3);

        xml.push_str("</");

        if with_xmlns {
            xml.push_str(Self::prefixed_name_or_name());
        } else {
            xml.push_str(Self::NAME);
        }

        xml.push('>');

        return xml;
    }

    #[inline]
    fn to_xml_string(&self, header: bool, with_xmlns: bool) -> String {
        let mut xml = String::with_capacity(64);

        if header {
            xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n");
        }

        xml.push_str(&self.xml_tag_start(with_xmlns));

        if let Some(xml_inner) = self.xml_inner(with_xmlns) {
            xml.push_str(&xml_inner);
        }

        xml.push_str(&self.xml_tag_end(with_xmlns));

        return xml;
    }

    #[inline]
    fn to_xml_bytes(&self, header: bool, with_xmlns: bool) -> Vec<u8> {
        let mut xml = Vec::with_capacity(128);

        if header {
            xml.extend_from_slice(
                b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n",
            );
        }

        xml.extend_from_slice(self.xml_tag_start(with_xmlns).as_bytes());

        if let Some(xml_inner) = self.xml_inner(with_xmlns) {
            xml.extend_from_slice(xml_inner.as_bytes());
        }

        xml.extend_from_slice(self.xml_tag_end(with_xmlns).as_bytes());

        return xml;
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct XmlNamespace {
    pub xmlns: Option<String>,
    pub xmlns_map: BTreeMap<String, String>,
    pub mc_ignorable: Option<String>,
}

impl XmlNamespace {
    pub fn serialize_attributes(&self, with_xmlns: bool) -> String {
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

        return attributes;
    }

    pub fn deserialize_attributes<'de>(
        &mut self,
        xml_reader: &mut impl XmlReader<'de>,
        attribute: &Attribute<'_>,
    ) -> Result<Option<()>, SdkErrorReport> {
        match attribute.key.0 {
            b"xmlns" => {
                self.xmlns = Some(
                    attribute
                        .decode_and_unescape_value(xml_reader.decoder())
                        .map_err(SdkError::from)?
                        .into_owned(),
                );
                Ok(Some(()))
            }
            b"mc:Ignorable" => {
                self.mc_ignorable = Some(
                    attribute
                        .decode_and_unescape_value(xml_reader.decoder())
                        .map_err(SdkError::from)?
                        .into_owned(),
                );
                Ok(Some(()))
            }
            other if let Some(ns) = other.strip_prefix(b"xmlns:") => {
                self.xmlns_map.insert(
                    String::from_utf8_lossy(ns).to_string(),
                    attribute
                        .decode_and_unescape_value(xml_reader.decoder())
                        .map_err(SdkError::from)?
                        .into_owned(),
                );
                Ok(Some(()))
            }
            _ => Ok(None),
        }
    }
}

pub fn resolve_zip_file_path(path: &str) -> String {
    let mut stack = Vec::new();

    for component in path.split('/') {
        match component {
            "" | "." => {
                // Ignore empty components and current directory symbol
            }
            ".." => {
                // Go up one directory if possible
                stack.pop();
            }
            _ => {
                // Add the component to the path
                stack.push(component);
            }
        }
    }
    // Join the components back into a path
    stack.join("/")
}

#[inline]
pub fn parse_bool_bytes(b: &[u8]) -> Result<bool, SdkErrorReport> {
    match b {
        b"true" | b"1" | b"True" | b"TRUE" | b"t" | b"Yes" | b"YES" | b"yes" | b"y" => Ok(true),
        b"false" | b"0" | b"False" | b"FALSE" | b"f" | b"No" | b"NO" | b"no" | b"n" | b"" => {
            Ok(false)
        }
        other => Err(SdkError::CommonError(
            String::from_utf8_lossy(other).into_owned(),
        ))?,
    }
}

#[inline]
pub fn as_xml_attribute(key: &str, value: &str) -> String {
    let mut attribute = String::with_capacity(16);

    attribute.push(' ');
    attribute.push_str(key);
    attribute.push_str("=\"");
    attribute.push_str(value);
    attribute.push('"');

    return attribute;
}

#[inline(always)]
pub(crate) fn expect_event_start<'de, T: Taggable>(
    xml_reader: &mut impl XmlReader<'de>,
    xml_event: Option<(BytesStart<'de>, bool)>,
) -> Result<(BytesStart<'de>, bool), SdkErrorReport> {
    if let Some((event, empty_tag)) = xml_event {
        return Ok((event, empty_tag));
    }

    let (event, empty_tag) = loop {
        let event = xml_reader.next()?;
        debug!("event: {event:?}");

        match event {
            Event::Start(b) => break (b, false),
            Event::Empty(b) => break (b, true),
            Event::Eof => {
                return Err(SdkError::UnknownError)
                    .attach(format!("Reached EOF when reading [{event:?}]"));
            }
            _ => continue,
        }
    };

    let event_name = event.name().0;
    if !T::matched_name(event_name) {
        let expected_tags = [Some(T::NAME), T::PREFIXED_NAME]
            .into_iter()
            .flatten()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let found_event_name = String::from_utf8_lossy(event_name).to_string();

        warn!("Mismatch: [{found_event_name}] does not match any of [{expected_tags}]");

        Err(SdkError::MismatchError {
            expected: format!("Any of [{expected_tags}]"),
            found: found_event_name,
        })?;
    }

    Ok((event, empty_tag))
}
