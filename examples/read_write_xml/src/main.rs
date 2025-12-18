use ooxmlsdk::{
    common::Deserializeable, schemas::schemas_openxmlformats_org_spreadsheetml_2006_main::Worksheet,
};

fn main() {
    let value = Worksheet::from_file("examples/read_write_xml/samples/sheet1.xml").unwrap();

    println!("{value}");
}
