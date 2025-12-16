use ooxmlsdk::{common::*, parts::wordprocessing_document::WordprocessingDocument};
use std::{fs::File, io::BufReader};

fn main() -> Result<(), SdkErrorReport> {
    let docx = WordprocessingDocument::new_from_file("examples/read_docx/samples/demo.docx")?;

    println!(
        "{}",
        docx.main_document_part
            .root_element
            .to_xml()
            .map_err(SdkError::from)?
    );

    println!("{}", docx.main_document_part.root_element.validate()?);

    let file = File::open("examples/read_docx/samples/demo.docx").map_err(SdkError::from)?;

    let reader = BufReader::new(file);

    let docx = WordprocessingDocument::new(reader)?;

    println!(
        "{}",
        docx.main_document_part
            .root_element
            .to_xml()
            .map_err(SdkError::from)?
    );

    println!("{}", docx.main_document_part.root_element.validate()?);

    Ok(())
}
