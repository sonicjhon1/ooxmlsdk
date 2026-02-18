use ooxmlsdk::{
    common::{Deserializeable, Serializeable},
    parts::{
        presentation_document::PresentationDocument, spreadsheet_document::SpreadsheetDocument,
        wordprocessing_document::WordprocessingDocument,
    },
    schemas::schemas_openxmlformats_org_spreadsheetml_2006_main::Worksheet,
};
use rootcause::{hooks::Hooks, prelude::*};
use rootcause_backtrace::BacktraceCollector;
use std::{
    fs::File,
    io::{BufReader, Cursor},
};
use tracing_subscriber::{EnvFilter, fmt};

const SAMPLE_DOCX_FILE_PATH: &str = "examples/samples/demo.docx";
const SAMPLE_PPTX_FILE_PATH: &str = "examples/samples/demo.pptx";
const SAMPLE_XLSX_FILE_PATH: &str = "examples/samples/demo.xlsx";
const SAMPLE_XML_FILE_PATH: &str = "examples/samples/sheet1.xml";

fn main() -> Result<(), Report> {
    let env_filter =
        EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("trace"));
    let subscriber = fmt().with_env_filter(env_filter);
    let _ = tracing::subscriber::set_global_default(subscriber.finish());

    Hooks::new()
        .report_creation_hook(BacktraceCollector::new_from_env())
        .install()?;

    {
        let docx = WordprocessingDocument::new_from_file(SAMPLE_DOCX_FILE_PATH)?;
        let docx_xml = docx
            .main_document_part
            .root_element
            .to_xml_string(true, true);
        println!("{docx_xml}");
        assert!(docx.main_document_part.root_element.validate()?);

        let reader = BufReader::new(File::open(SAMPLE_DOCX_FILE_PATH)?);
        let reader_docx = WordprocessingDocument::new(reader)?;
        assert_eq!(
            reader_docx
                .main_document_part
                .root_element
                .to_xml_string(true, true),
            docx_xml
        );

        let temp_docx_file = Cursor::new(Vec::new());
        docx.save(temp_docx_file)?;
    }

    {
        let pptx = PresentationDocument::new_from_file(SAMPLE_PPTX_FILE_PATH)?;
        let pptx_xml = pptx
            .presentation_part
            .root_element
            .to_xml_string(true, true);
        println!("{pptx_xml}");
        assert!(pptx.presentation_part.root_element.validate()?);

        let reader = BufReader::new(File::open(SAMPLE_PPTX_FILE_PATH)?);
        let reader_pptx = PresentationDocument::new(reader)?;
        assert_eq!(
            reader_pptx
                .presentation_part
                .root_element
                .to_xml_string(true, true),
            pptx_xml
        );

        let temp_pptx_file = Cursor::new(Vec::new());
        pptx.save(temp_pptx_file)?;
    }

    {
        let xlsx = SpreadsheetDocument::new_from_file(SAMPLE_XLSX_FILE_PATH).unwrap();
        let xlsx_xml = xlsx.workbook_part.root_element.to_xml_string(true, true);
        println!("{xlsx_xml}");
        assert!(xlsx.workbook_part.root_element.validate()?);

        let reader = BufReader::new(File::open(SAMPLE_XLSX_FILE_PATH)?);
        let reader_xlsx = SpreadsheetDocument::new(reader)?;
        assert_eq!(
            reader_xlsx
                .workbook_part
                .root_element
                .to_xml_string(true, true),
            xlsx_xml
        );

        let temp_xlsx_file = Cursor::new(Vec::new());
        xlsx.save(temp_xlsx_file)?;
    }

    {
        let worksheet = Worksheet::from_file(SAMPLE_XML_FILE_PATH).unwrap();
        let xml = worksheet.to_xml_string(true, true);
        println!("{xml}");
    }

    Ok(())
}
