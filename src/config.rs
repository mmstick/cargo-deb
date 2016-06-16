use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use rustc_serialize;
use toml;

#[derive(Debug)]
pub struct Config {
    /// The name of the project to build
    pub name: String,
    /// The version number of the project.
    pub version: String,
    /// A short description of the project.
    pub description: String,
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
        .map_or_else(|| panic!("cargo-deb: could not open manifest file"), |mut file| {
            // Read the contents of the Cargo.toml fie into the `content` String
            file.read_to_string(content)
                // Panic if Cargo.toml could not be opened.
                .expect("cargo-deb: invalid or missing Cargo.toml options");
        });
}

/// Utilizes `dpkg --print-architecutre` to determine that architecture to generate a package for.
fn get_arch() -> String {
    let output = Command::new("dpkg").arg("--print-architecture").output()
        .expect("cargo-deb: failed to run 'dpkg --print-architecture'");
    let mut arch = String::from_utf8(output.stdout)
        .expect("cargo-deb: 'dpkg --print-architecture' did not return a valid UTF8 string.");
    arch.pop().unwrap();
    arch
}
