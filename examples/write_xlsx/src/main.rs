use ooxmlsdk::{common::*, parts::spreadsheet_document::SpreadsheetDocument};
use std::fs::OpenOptions;

fn main() -> Result<(), SdkErrorReport> {
    let xlsx = SpreadsheetDocument::new_from_file("examples/read_xlsx/samples/demo.xlsx")?;

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("/tmp/demo.xlsx")
        .map_err(SdkError::from)?;

    xlsx.save(file)?;

    Ok(())
}
