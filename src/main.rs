extern crate itertools;
extern crate libc;
extern crate toml;
extern crate tar;
extern crate lzma;
extern crate zopfli;
extern crate md5;
extern crate file;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate getopts;

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
const TAR_REJECTS_CUR_DIR: bool = true;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut cli_opts = getopts::Options::new();
    cli_opts.optflag("", "no-build", "Assume project is already built");
    cli_opts.optflag("", "no-strip", "Do not strip debug symbols from the binary");
    cli_opts.optflag("h", "help", "Print this help menu");

    let matches = match cli_opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            failed(e.to_string());
        },
    };
    if matches.opt_present("h") {
        print!("{}", cli_opts.usage("Usage: cargo deb [options]"));
        return;
    }
    let no_build = matches.opt_present("no-build");
    let no_strip = matches.opt_present("no-strip");

    remove_leftover_files();
    let options = Config::new();
    if !no_build {
        cargo_build(&options.features, options.default_features);
    }
    if options.strip && !no_strip {
        strip_binary(options.name.as_str());
    }

    // Obtain the current time which will be used to stamp the generated files in the archives.
    let system_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH)
        .try("unable to get system time").as_secs();

    // Initailize the contents of the data archive (files that go into the filesystem).
    let mut data_archive = TarBuilder::new(Vec::new());
    let asset_hashes = data::generate_archive(&mut data_archive, &options, system_time);

    // Initialize the contents of the control archive (metadata for the package manager).
    let mut control_archive = TarBuilder::new(Vec::new());
    control::generate_archive(&mut control_archive, &options, system_time, asset_hashes);

    // Compress the data archive with the LZMA compression algorithm.
    {
        let tar = data_archive.into_inner().try("failed to tar contents");
        if let Err(reason) = compress::xz(tar, "target/debian/data.tar.xz") {
            compress::exit_with(reason);
        }
    }

    // Compress the control archive with the Zopfli compression algorithm.
    {
        let tar = control_archive.into_inner().try("failed to tar contents");
        if let Err(reason) = compress::gz(tar, "target/debian/control.tar.gz") {
            compress::exit_with(reason);
        }
    }

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
        .try("unable to create debian archive");
}

// Creates the debian-binary file that will be added to the final ar archive.
fn generate_debian_binary_file() {
    let mut file = fs::OpenOptions::new().create(true).write(true)
        .truncate(true).mode(CHMOD_FILE).open("target/debian/debian-binary")
        .try("unable to create target/debian/debian-binary");
    file.write(&[50, 46, 48, 10]).unwrap(); // [2][.][0][BS]
}

/// Removes the target/debian directory so that we can start fresh.
fn remove_leftover_files() {
    let _ = fs::remove_dir_all("target/debian");
    fs::create_dir_all("target/debian").try("unable to create debian target");
}

/// Builds a release binary with `cargo build --release`
fn cargo_build(features: &[String], default_features: bool) {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--release");

    if !default_features {
        cmd.arg("--no-default-features");
    }
    if !features.is_empty() {
        cmd.arg(format!("--features={}", features.join(",")));
    }

    let status = cmd.status().try("failed to build project");
    if !status.success() {
        failed("build failed");
    }
}

// Strips the binary that was created with cargo
fn strip_binary(name: &str) {
    let status = Command::new("strip").arg("--strip-unneeded").arg(String::from("target/release/") + name)
        .status().try("could not strip binary");
    if !status.success() {
        failed("strip failed");
    }
}
