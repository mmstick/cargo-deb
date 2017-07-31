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
pub struct Asset {
    pub source_file: String,
    pub target_path: String,
    pub chmod: u32,
}

#[derive(Debug)]
pub struct Config {
    /// The name of the project to build
    pub name: String,
    /// The software license of the project.
    pub license: String,
    /// The location of the license file followed by the amount of lines to skip.
    pub license_file: Option<String>,
    pub license_file_skip_lines: usize,
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
    pub section: Option<String>,
    /// The priority of the project. Typically 'optional'.
    pub priority: String,
    /// The architecture of the running system.
    pub architecture: String,
    /// A list of configuration files installed by the package.
    pub conf_files: Option<String>,
    /// All of the files that are to be packaged.
    pub assets: Vec<Asset>,
    /// The path were possible maintainer scripts live
    pub maintainer_scripts: Option<PathBuf>,
    /// List of Cargo features to use during build
    pub features: Vec<String>,
    pub default_features: bool,
}

impl Config {
    pub fn new() -> Config {
        let mut content = String::new();
        manifest_contents(&current_manifest_path(), &mut content);
        toml::from_str::<Cargo>(&content).try("cargo-deb: could not decode manifest").into_config()
    }

    pub fn get_dependencies(&self) -> String {
        self.depends.split_whitespace().map(|word| match word {
            "$auto"  => resolve(String::from("target/release/") + &self.name),
            "$auto," => resolve(String::from("target/release/") + &self.name + ","),
            _        => word.to_owned()
        }).join(" ")
    }
}


#[derive(Clone, Debug, Deserialize)]
pub struct Cargo {
    pub package: CargoPackage,
}

impl Cargo {
    fn into_config(mut self) -> Config {
        let (license_file, license_file_skip_lines) = if let Some(mut args) = self.package.metadata.deb.license_file.take() {
            let mut args = args.drain(..);
            (args.next(), args.next().map(|p|p.parse().try("invalid number of lines to skip")).unwrap_or(0))
        } else {
            (None, 0)
        };
        Config {
            name: self.package.name.clone(),
            license: self.package.license.clone(),
            license_file,
            license_file_skip_lines,
            copyright: self.package.metadata.deb.copyright.take().unwrap_or_else(|| {
                self.package.authors.as_ref().try("Package must have a copyright or authors").join(", ")
            }),
            version: self.version_string(),
            homepage: self.package.homepage.clone(),
            repository: self.package.repository.clone(),
            description: self.package.description.clone(),
            extended_description: self.package.metadata.deb.extended_description.take()
                .map(|d|d.split_by_chars(79)).unwrap_or(vec![]),
            maintainer: self.package.metadata.deb.maintainer.take().unwrap_or_else(|| {
                self.package.authors.as_ref().and_then(|a|a.get(0))
                    .try("Package must have a maintainer or authors").to_owned()
            }),
            depends: self.package.metadata.deb.depends.take().unwrap_or("$auto".to_owned()),
            section: self.package.metadata.deb.section.take(),
            priority: self.package.metadata.deb.priority.take().unwrap_or("optional".to_owned()),
            architecture: get_arch().to_owned(),
            conf_files: self.package.metadata.deb.conf_files.clone()
                .map(|x| x.iter().fold(String::new(), |a, b| a + b + "\n")),
            assets: self.take_assets(),
            maintainer_scripts: self.package.metadata.deb.maintainer_scripts.clone().map(|s| PathBuf::from(s)),
            features: self.package.metadata.deb.features.take().unwrap_or(vec![]),
            default_features: self.package.metadata.deb.default_features.unwrap_or(true),
        }
    }

    fn take_assets(&mut self) -> Vec<Asset> {
        if let Some(assets) = self.package.metadata.deb.assets.take() {
            assets.into_iter().map(|mut v| {
                let mut v = v.drain(..);
                Asset {
                    source_file: v.next().try("missing path for asset"),
                    target_path: v.next().try("missing target for asset"),
                    chmod: u32::from_str_radix(&v.next().try("missing chmod for asset"), 8)
                        .try("unable to parse chmod argument"),
                }
            }).collect()
        } else {
            vec![]
        }
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
    pub authors: Option<Vec<String>>,
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
#[serde(rename_all = "kebab-case")]
pub struct CargoDeb {
    pub maintainer: Option<String>,
    pub copyright: Option<String>,
    pub license_file: Option<Vec<String>>,
    pub depends: Option<String>,
    pub extended_description: Option<String>,
    pub section: Option<String>,
    pub priority: Option<String>,
    pub revision: Option<String>,
    pub conf_files: Option<Vec<String>>,
    pub assets: Option<Vec<Vec<String>>>,
    pub maintainer_scripts: Option<String>,
    pub features: Option<Vec<String>>,
    pub default_features: Option<bool>,
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
