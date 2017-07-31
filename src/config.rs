use std::fs::File;
use std::io::Read;
use std::env::consts::ARCH;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use itertools::Itertools;
use toml;
use dependencies::resolve;
use serde_json;

use wordsplit::WordSplit;
use try::Try;

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
    /// The homepage of the project.
    pub homepage: Option<String>,
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
    /// A list of configuration files installed by the package.
    pub conf_files: Option<String>,
    /// All of the files that are to be packaged. `{ source_file, target_path, chmod }`
    pub assets: Vec<Vec<String>>,
    /// The path were possible maintainer scripts live
    pub maintainer_scripts: Option<PathBuf>,
}

impl Config {
    pub fn new() -> Config {
        let mut content = String::new();
        manifest_contents(&current_manifest_path(), &mut content);
        toml::from_str::<Cargo>(&content).try("cargo-deb: could not decode manifest").to_config()
    }
}


#[derive(Clone, Debug, Deserialize)]
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
            version: self.version_string(),
            homepage: self.package.homepage.clone(),
            repository: self.package.repository.clone(),
            description: self.package.description.clone(),
            extended_description: self.package.metadata.deb.extended_description.split_by_chars(79),
            maintainer: self.package.metadata.deb.maintainer.clone(),
            depends: self.get_dependencies(&self.package.metadata.deb.depends),
            section: self.package.metadata.deb.section.clone(),
            priority: self.package.metadata.deb.priority.clone(),
            architecture: get_arch().to_owned(),
            conf_files: self.package.metadata.deb.conf_files.clone()
                .map(|x| x.iter().fold(String::new(), |a, b| a + b + "\n")),
            assets: self.package.metadata.deb.assets.clone(),
            maintainer_scripts: self.package.metadata.deb.maintainer_scripts.clone().map(|s| PathBuf::from(s))
        }
    }

    fn get_dependencies(&self, input: &str) -> String {
        input.split_whitespace().map(|word| match word {
            "$auto"  => resolve(String::from("target/release/") + &self.package.name),
            "$auto," => resolve(String::from("target/release/") + &self.package.name + ","),
            _        => word.to_owned()
        }).join(" ")
    }

    fn version_string(&self) -> String {
        if let Some(ref revision) = self.package.metadata.deb.revision {
            format!("{}-{}", self.package.version, revision)
        } else {
            self.package.version.clone()
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoPackage {
    pub name: String,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: String,
    pub version: String,
    pub description: String,
    pub metadata: CargoMetadata
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoMetadata {
    pub deb: CargoDeb
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoDeb {
    pub maintainer: String,
    pub copyright: String,
    pub license_file: Vec<String>,
    pub depends: String,
    pub extended_description: String,
    pub section: String,
    pub priority: String,
    pub revision: Option<String>,
    pub conf_files: Option<Vec<String>>,
    pub assets: Vec<Vec<String>>,
    pub maintainer_scripts: Option<String>
}

/// Returns the path of the `Cargo.toml` that we want to build.
fn current_manifest_path() -> PathBuf {
    let output = Command::new("cargo").arg("locate-project").output()
        .try("cargo-deb: unable to obtain output of `cargo locate-proect`");
    if !output.status.success() { exit(output.status.code().unwrap_or(-1)); }

    #[derive(Deserialize)]
    struct Data { root: String }
    let stdout = String::from_utf8(output.stdout).unwrap();
    let decoded: Data = serde_json::from_str(&stdout).unwrap();
    Path::new(&decoded.root).to_owned()
}

/// Opens the Cargo.toml file and places the contents into the `content` `String`.
fn manifest_contents(manifest_path: &Path, content: &mut String) {
    File::open(manifest_path).try("cargo-deb: could not open manifest file")
        .read_to_string(content).try("cargo-deb: invalid or missing Cargo.toml options");
}

/// Calls the `uname` function from libc to obtain the machine architecture,
/// and then Debianizes the architecture name.
fn get_arch() -> &'static str {
    match ARCH {
        "arm"     => "armhf",
        "aarch64" => "arm64",
        "x86_64"  => "amd64",
        "noarch"  => "all",
        _         => ARCH
    }
}
