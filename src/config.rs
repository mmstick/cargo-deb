use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::mem;
use libc;
use rustc_serialize;
use toml;

use wordsplit::WordSplit;
use try::{failed, Try};

#[derive(Debug)]
pub struct Config {
    /// The name of the project to build
    pub name: String,
    /// The software license of the project.
    pub license: String,
    /// The location of the license file followed by the amount of lines to skip.
    pub license_file: Vec<String>,
    /// The copyright of the project.
    pub copyright: String,
    /// The version number of the project.
    pub version: String,
    /// The URL of the software repository.
    pub repository: String,
    /// A short description of the project.
    pub description: String,
    /// An extended description of the project.
    pub extended_description: Vec<String>,
    /// The maintainer of the Debian package.
    pub maintainer: String,
    /// The Debian dependencies required to run the project.
    pub depends: String,
    /// The category by which the package belongs.
    pub section: String,
    /// The priority of the project. Typically 'optional'.
    pub priority: String,
    /// The architecture of the running system.
    pub architecture: String,
    /// All of the files that are to be packaged. `{ source_file, target_path, chmod }`
    pub assets: Vec<Vec<String>>
}

impl Config {
    pub fn new() -> Config {
        let mut content = String::new();
        manifest_contents(&current_manifest_path(), &mut content);
        toml::decode_str::<Cargo>(&content).try("cargo-deb: could not decode manifest").to_config()
    }
}


#[derive(Clone, Debug, RustcDecodable)]
pub struct Cargo {
    pub package: CargoPackage,
}

impl Cargo {
    fn to_config(&self) -> Config {
        Config {
            name: self.package.name.clone(),
            license: self.package.license.clone(),
            license_file: self.package.metadata.deb.license_file.clone(),
            copyright: self.package.metadata.deb.copyright.clone(),
            version: self.package.version.clone(),
            repository: self.package.repository.clone(),
            description: self.package.description.clone(),
            extended_description: self.package.metadata.deb.extended_description.split_by_chars(79),
            maintainer: self.package.metadata.deb.maintainer.clone(),
            depends: self.package.metadata.deb.depends.clone(),
            section: self.package.metadata.deb.section.clone(),
            priority: self.package.metadata.deb.priority.clone(),
            architecture: get_arch(),
            assets: self.package.metadata.deb.assets.clone(),
        }
    }
}

#[derive(Clone, Debug, RustcDecodable)]
pub struct CargoPackage {
    pub name: String,
    pub license: String,
    pub repository: String,
    pub version: String,
    pub description: String,
    pub metadata: CargoMetadata
}

#[derive(Clone, Debug, RustcDecodable)]
pub struct CargoMetadata {
    pub deb: CargoDeb
}

#[derive(Clone, Debug, RustcDecodable)]
pub struct CargoDeb {
    pub maintainer: String,
    pub copyright: String,
    pub license_file: Vec<String>,
    pub depends: String,
    pub extended_description: String,
    pub section: String,
    pub priority: String,
    pub assets: Vec<Vec<String>>,
}

/// Returns the path of the `Cargo.toml` that we want to build.
fn current_manifest_path() -> PathBuf {
    let output = Command::new("cargo").arg("locate-project").output().unwrap();

    if !output.status.success() { exit(output.status.code().unwrap_or(-1)); }

    #[derive(RustcDecodable)]
    struct Data { root: String }
    let stdout = String::from_utf8(output.stdout).unwrap();
    let decoded: Data = rustc_serialize::json::decode(&stdout).unwrap();
    Path::new(&decoded.root).to_owned()
}

/// Opens the Cargo.toml file and places the contents into the `content` `String`.
fn manifest_contents(manifest_path: &Path, content: &mut String) {
    File::open(manifest_path).ok()
        // If Cargo.toml cannot be opened, panic.
        .map_or_else(|| failed("cargo-deb: could not open manifest file"), |mut file| {
            // Read the contents of the Cargo.toml fie into the `content` String
            file.read_to_string(content)
                // Error if Cargo.toml could not be opened.
                .try("cargo-deb: invalid or missing Cargo.toml options");
        });
}

/// Calls the `uname` function from libc to obtain the machine architecture, and then Debianizes the architecture name.
fn get_arch() -> String {
    let arch = unsafe {
        let mut utsname: libc::utsname = mem::uninitialized();
        let status = libc::uname(&mut utsname);
        if status < 0 {
            failed("cargo-deb: could not obtain machine architecture from the libc uname function");
        } else {
            String::from_utf8_unchecked(utsname.machine.iter().map(|x| *x as u8).collect::<Vec<u8>>())
        }
    };

    match arch.as_str() {
        "x86_64" => String::from("amd64"),
        "noarch" => String::from("all"),
        _        => arch
    }
}
