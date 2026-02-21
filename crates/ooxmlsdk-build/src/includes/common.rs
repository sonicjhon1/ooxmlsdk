use quick_xml::{
    Decoder, Reader,
    escape::{escape, unescape},
    events::{BytesEnd, BytesRef, BytesStart, BytesText, Event, attributes::Attribute},
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
    #[error("EscapeError")]
    EscapeError(#[from] quick_xml::escape::EscapeError),
    #[error("Utf8Error")]
    Utf8Error(#[from] std::str::Utf8Error),
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

pub struct SliceReader<'de> {
    reader: Reader<&'de [u8]>,
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

    fn matched_bytes_start(bytes_start: &BytesStart<'_>) -> bool {
        Self::matched_name(bytes_start.name().0)
    }

    fn matched_bytes_end(bytes_end: &BytesEnd<'_>) -> bool {
        Self::matched_name(bytes_end.name().0)
    }

    fn matched_name(bytes: &[u8]) -> bool {
        let prefixed_bytes = Self::PREFIXED_NAME.map(|s| s.as_bytes());

        trace!(
            "Trying to match bytes: ({}) with ({}) or ({:?})",
            String::from_utf8_lossy(bytes),
            String::from_utf8_lossy(Self::NAME.as_bytes()),
            prefixed_bytes.map(String::from_utf8_lossy)
        );

        return bytes == Self::NAME.as_bytes() || Some(bytes) == prefixed_bytes;
    }
}

pub trait Deserializeable: Taggable + Sized {
    fn from_str(str: impl AsRef<str>) -> Result<Self, SdkErrorReport>
    where
        Self: Default, {
        let mut xml_reader = quick_xml::Reader::from_str(str.as_ref());
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_from_xml_reader(&mut SliceReader::new(xml_reader))
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, SdkErrorReport>
    where
        Self: Default, {
        let mut xml_reader = quick_xml::Reader::from_reader(bytes);
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_from_xml_reader(&mut SliceReader::new(xml_reader))
    }

    fn from_reader(reader: impl BufRead) -> Result<Self, SdkErrorReport>
    where
        Self: Default, {
        let mut xml_reader = quick_xml::Reader::from_reader(reader);
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_from_xml_reader(&mut IoReader::new(xml_reader))
    }

    fn from_file(path: impl AsRef<Path>) -> Result<Self, SdkErrorReport>
    where
        Self: Default, {
        let mut xml_reader = quick_xml::Reader::from_file(path).map_err(SdkError::from)?;
        xml_reader.config_mut().check_end_names = false;
        xml_reader.config_mut().trim_text(false);

        Self::deserialize_from_xml_reader(&mut IoReader::new(xml_reader))
    }

    fn deserialize_from_xml_reader<'de>(
        xml_reader: &mut impl XmlReader<'de>,
    ) -> Result<Self, SdkErrorReport>
    where
        Self: Default, {
        let (bytes_start, is_empty) = BytesEvent::expect_taggable_start::<Self>(xml_reader)?;
        let mut output = Self::default().deserialize_attributes(xml_reader, bytes_start)?;

        if !is_empty {
            output = output.deserialize_children(xml_reader)?;
        }

        return Ok(output);
    }

    fn deserialize_attributes<'de>(
        self,
        _xml_reader: &impl XmlReader<'de>,
        _xml_event: BytesStart<'de>,
    ) -> Result<Self, SdkErrorReport> {
        tracing::warn!(
            "({})'s deserialize_attributes uses the default no-op impl",
            Self::prefixed_name_or_name()
        );
        Ok(self)
    }

    fn deserialize_children<'de>(
        self,
        _xml_reader: &mut impl XmlReader<'de>,
    ) -> Result<Self, SdkErrorReport> {
        tracing::warn!(
            "({})'s deserialize_children uses the default no-op impl",
            Self::prefixed_name_or_name()
        );
        Ok(self)
    }
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
        xml_reader: &impl XmlReader<'de>,
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct XmlContent<T = String>(pub T);

impl XmlContent<String> {
    pub fn from_unescaped(str: impl AsRef<str>) -> Self { Self(escape(str.as_ref()).to_string()) }

    pub fn unescaped(&self) -> Result<String, SdkErrorReport> {
        Ok(unescape(self.0.as_str())
            .map_err(SdkError::from)
            .attach_with(|| self.0.to_owned())?
            .to_string())
    }

    pub fn append_escaped_ref_bytes(&mut self, ref_bytes: &[u8]) -> Result<(), SdkErrorReport> {
        let ref_str = format!("&{};", str::from_utf8(ref_bytes).map_err(SdkError::from)?);

        self.0.push_str(&ref_str);

        Ok(())
    }

    pub fn append_escaped_bytes(&mut self, bytes: &[u8]) -> Result<(), SdkErrorReport> {
        let str = str::from_utf8(bytes).map_err(SdkError::from)?;

        self.0.push_str(str);

        Ok(())
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

#[derive(Debug)]
pub enum BytesEvent<'de> {
    BytesStart(BytesStart<'de>, bool),
    BytesText(BytesText<'de>),
    BytesRef(BytesRef<'de>),
    End(BytesEnd<'de>),
}

impl<'de> BytesEvent<'de> {
    #[inline]
    #[instrument(skip_all)]
    pub fn expect_taggable_start<T: Taggable>(
        xml_reader: &mut impl XmlReader<'de>,
    ) -> Result<(BytesStart<'de>, bool), Report<SdkError>> {
        loop {
            let expect = Self::expect(xml_reader)?;

            match expect {
                BytesEvent::BytesStart(bytes_start, is_empty) => {
                    debug_assert!(
                        T::matched_bytes_start(&bytes_start),
                        "Expected ({}), found ({})",
                        T::prefixed_name_or_name(),
                        String::from_utf8_lossy(bytes_start.name().0)
                    );

                    return Ok((bytes_start, is_empty));
                }
                BytesEvent::BytesText(..) => {}
                BytesEvent::BytesRef(..) => {}
                BytesEvent::End(..) => unreachable!(),
            }
        }
    }

    #[inline]
    #[instrument(skip_all)]
    pub fn expect(xml_reader: &mut impl XmlReader<'de>) -> Result<Self, Report<SdkError>> {
        loop {
            let event = xml_reader.next()?;
            debug!("Event: {event:?}");

            match event {
                Event::Start(bytes_start) => {
                    tracing::debug!("Matched Start: ({})", String::from_utf8_lossy(&bytes_start));
                    return Ok(Self::BytesStart(bytes_start, false));
                }
                Event::Empty(bytes_start) => {
                    tracing::debug!("Matched Empty: ({})", String::from_utf8_lossy(&bytes_start));
                    return Ok(Self::BytesStart(bytes_start, true));
                }
                Event::Text(bytes_text) => {
                    tracing::debug!("Matched Text: ({})", String::from_utf8_lossy(&bytes_text));
                    return Ok(Self::BytesText(bytes_text));
                }
                Event::GeneralRef(bytes_ref) => {
                    tracing::debug!("Matched Ref: ({})", String::from_utf8_lossy(&bytes_ref));
                    return Ok(Self::BytesRef(bytes_ref));
                }
                Event::End(bytes_end) => {
                    tracing::debug!("Matched End: ({})", String::from_utf8_lossy(&bytes_end));
                    return Ok(Self::End(bytes_end));
                }
                Event::Eof => {
                    return Err(SdkError::UnknownError)
                        .attach(format!("Reached EOF when reading [{event:?}]"));
                }
                _ => continue,
            };
        }
    }
}
