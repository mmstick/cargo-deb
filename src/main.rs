extern crate itertools;
extern crate libc;
extern crate rustc_serialize;
extern crate toml;
extern crate tar;
extern crate lzma;
extern crate zopfli;

mod compress;
mod config;
mod control;
mod data;
mod dependencies;
mod try;
mod wordsplit;

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::time;
use std::os::unix::fs::OpenOptionsExt;

use config::Config;
use try::{failed, Try};
use tar::Builder as TarBuilder;

const CHMOD_FILE: u32 = 420;

fn main() {
    remove_leftover_files();
    if !std::env::args().any(|x| x.as_str() == "--no-build") {
        cargo_build();
    }
    let options = Config::new();
    strip_binary(options.name.as_str());

    // Obtain current time
    let system_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH)
        .try("cargo-deb: unable to get system time").as_secs();

    // Initailize the data archive
    let mut data_archive = TarBuilder::new(Vec::new());
    data::generate_archive(&mut data_archive, &options, &system_time);

    // Initialize the control archive
    let mut control_archive = TarBuilder::new(Vec::new());
    control::generate_archive(&mut control_archive, &options, &system_time);

    // Compress the archives
    data_archive.into_inner()
        .map(|tar| compress::xz(tar, "target/debian/data.tar.xz"))
        .try("cargo-deb: failed to tar contents");
    control_archive.into_inner()
        .map(|tar| compress::gz(tar, "target/debian/control.tar.gz"))
        .try("cargo-deb: failed to tar contents");
    generate_debian_binary_file();
    generate_deb(&options);
}

/// Uses the ar program to create the final Debian package, at least until a native ar implementation is implemented.
fn generate_deb(config: &Config) {
    env::set_current_dir("target/debian").unwrap();
    let outpath = config.name.clone() + "_" + &config.version + "_" +
        &config.architecture + ".deb";
    let _ = fs::remove_file(&outpath); // Remove it if it exists
    Command::new("ar").arg("r").arg(outpath).arg("debian-binary").arg("control.tar.gz").arg("data.tar.xz").status()
        .try("cargo-deb: unable to create debian archive");
}

// Creates the debian-binary file that will be added to the final ar archive.
fn generate_debian_binary_file() {
    fs::OpenOptions::new().create(true).write(true).truncate(true).mode(CHMOD_FILE)
        .open("target/debian/debian-binary").ok()
        .map_or_else(|| failed("cargo-deb: unable to create target/debian/debian-binary"), |mut file| {
            file.write(&[50, 46, 48, 10]).unwrap(); // [2][.][0][BS]
        });
}

/// Removes the target/debian directory so that we can start fresh.
fn remove_leftover_files() {
    let _ = fs::remove_dir_all("target/debian");
    fs::create_dir_all("target/debian").try("cargo-deb: unable to create debian target");
}

/// Builds a release binary with `cargo build --release`
fn cargo_build() {
    Command::new("cargo").arg("build").arg("--release").status().try("cargo-deb: failed to build project");
}

// Strips the binary that was created with cargo
fn strip_binary(name: &str) {
    Command::new("strip").arg("--strip-unneeded").arg(String::from("target/release/") + name)
        .status().try("cargo-deb: could not strip binary");
}
