use quick_xml::{
    Decoder, Reader,
    events::{BytesStart, Event},
};
use rootcause::prelude::*;
use std::{io::BufRead, path::Path};
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

pub trait Deserializeable: Sized {
    fn from_str(str: impl AsRef<str>) -> Result<Self, SdkErrorReport> {
        let mut xml_reader = quick_xml::Reader::from_str(str.as_ref());
        xml_reader.config_mut().check_end_names = false;

        Self::deserialize_inner(&mut SliceReader::new(xml_reader), None)
    }

    fn from_reader(reader: impl BufRead) -> Result<Self, SdkErrorReport> {
        let mut xml_reader = quick_xml::Reader::from_reader(reader);
        xml_reader.config_mut().check_end_names = false;

        Self::deserialize_inner(&mut IoReader::new(xml_reader), None)
    }

    fn from_file(path: impl AsRef<Path>) -> Result<Self, SdkErrorReport> {
        let mut xml_reader = quick_xml::Reader::from_file(path).map_err(SdkError::from)?;
        xml_reader.config_mut().check_end_names = false;

        Self::deserialize_inner(&mut IoReader::new(xml_reader), None)
    }

    fn deserialize_inner<'de>(
        xml_reader: &mut impl XmlReader<'de>,
        xml_event: Option<(BytesStart<'de>, bool)>,
    ) -> Result<Self, SdkErrorReport>;
}

pub trait Serializeable {
    const PREFIXED_NAME: &str;

    const NAME: &str;

    fn xml_tag_attributes(&self, with_xmlns: bool) -> Option<String>;

    fn xml_inner(&self, with_xmlns: bool) -> Option<String>;

    #[inline]
    fn xml_tag_start(&self, with_xmlns: bool) -> String {
        let mut xml = String::with_capacity(const { Self::PREFIXED_NAME.len() + 32 });

        xml.push('<');

        if with_xmlns {
            xml.push_str(Self::PREFIXED_NAME);
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
        let mut xml = String::with_capacity(const { Self::PREFIXED_NAME.len() + 3 });

        xml.push_str("</");

        if with_xmlns {
            xml.push_str(Self::PREFIXED_NAME);
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
pub(crate) fn expect_event_start<'de>(
    xml_reader: &mut impl XmlReader<'de>,
    xml_event: Option<(BytesStart<'de>, bool)>,
    tag_prefixed: &[u8],
    tag: &[u8],
) -> Result<(BytesStart<'de>, bool), SdkErrorReport> {
    debug!("xml_event: {:?}", xml_event);

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

    debug!("({event:?}, {empty_tag})");

    let event_name = event.name().0;
    if !(event_name == tag_prefixed || event_name == tag) {
        let expected_tag_prefixed = String::from_utf8_lossy(tag_prefixed).to_string();
        let expected_tag = String::from_utf8_lossy(tag).to_string();
        let found_event_name = String::from_utf8_lossy(event_name).to_string();

        warn!(
            "Mismatch: [{found_event_name}] does not match [{expected_tag_prefixed}] OR [{expected_tag}]"
        );

        Err(SdkError::MismatchError {
            expected: format!("{expected_tag_prefixed} OR {expected_tag}"),
            found: found_event_name,
        })?;
    }

    Ok((event, empty_tag))
}
