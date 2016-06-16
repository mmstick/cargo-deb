use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use rustc_serialize;
use toml;

#[derive(Debug)]
pub struct Config {
    pub name: String,
    pub version: String,
    pub description: String,
    pub maintainer: String,
    pub depends: String,
    pub section: String,
    pub priority: String,
    pub architecture: String,
    pub assets: Vec<Vec<String>>
}

impl Config {
    pub fn new() -> Config {
        let mut content = String::new();
        manifest_contents(&current_manifest_path(), &mut content);
        toml::decode_str::<Cargo>(&content).expect("cargo-deb: could not decode manifest").to_config()
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
            version: self.package.version.clone(),
            description: self.package.description.clone(),
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
    pub depends: String,
    pub section: String,
    pub priority: String,
    pub assets: Vec<Vec<String>>,
}

/// Returns the path of the `Cargo.toml` that we want to build.
fn current_manifest_path() -> PathBuf {
    let output = Command::new("cargo").arg("locate-project").output().unwrap();

    if !output.status.success() {
        if let Some(code) = output.status.code() {
            exit(code);
        } else {
            exit(-1);
        }
    }

    #[derive(RustcDecodable)]
    struct Data { root: String }
    let stdout = String::from_utf8(output.stdout).unwrap();
    let decoded: Data = rustc_serialize::json::decode(&stdout).unwrap();
    Path::new(&decoded.root).to_owned()
}

fn manifest_contents(manifest_path: &Path, content: &mut String) {
    File::open(manifest_path).ok()
        .map_or_else(|| panic!("cargo-deb: could not open manifest file"), |mut file| {
            file.read_to_string(content).expect("cargo-deb: invalid or missing Cargo.toml options");
        });
}

fn get_arch() -> String {
    String::from("amd64")
}
