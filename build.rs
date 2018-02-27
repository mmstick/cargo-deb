extern crate file;
use std::env;

// env::ARCH doesn't include full triple, and AFAIK there isn't a nicer way of getting the full triple
// (see lib.rs for the rest of this hack)
fn main() {
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("default_target.rs");
    file::put_text(out, env::var("TARGET").unwrap()).unwrap();

    println!("cargo:rerun-if-changed=build.rs"); // optimization: avoid re-running this script
}
