use ooxmlsdk_build::generate;
use std::env;

fn main() {
  let out_dir = env::var("OUT_DIR").unwrap();

  generate(&out_dir);
}
