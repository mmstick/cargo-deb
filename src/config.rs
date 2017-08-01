use std::env::consts::ARCH;
use std::path::PathBuf;
use std::process::Command;
use toml;
use file;
use dependencies::resolve;
use serde_json;
use error::*;

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
    pub documentation: Option<String>,
    /// The URL of the software repository.
    pub repository: Option<String>,
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
    /// Should the binary be stripped from debug symbols?
    pub strip: bool,
}

impl Config {
    pub fn from_manifest() -> CDResult<Config> {
        let manifest_path = current_manifest_path()?;
        let content = file::get_text(&manifest_path)
            .map_err(|e| CargoDebError::IoFile(e, manifest_path))?;
        toml::from_str::<Cargo>(&content)?.into_config()
    }

    pub fn get_dependencies(&self) -> CDResult<String> {
        let deps: Result<Vec<_>,_> = self.depends.split_whitespace().map(|word| match word {
            "$auto"  => resolve(&format!("target/release/{}", &self.name)),
            "$auto," => resolve(&format!("target/release/{},", &self.name)),
            _        => Ok(word.to_owned())
        }).collect();
        Ok(deps?.join(" "))
    }

    /// Tries to guess type of source control used for the repo URL.
    /// It's a guess, and it won't be 100% accurate, because Cargo suggests using
    /// user-friendly URLs or webpages instead of tool-specific URL schemes.
    pub fn repository_type(&self) -> Option<&str> {
        if let Some(ref repo) = self.repository {
            if repo.starts_with("git+") || repo.ends_with(".git") || repo.contains("git@") || repo.contains("github.com") || repo.contains("gitlab.com") {
                return Some("Git");
            }
            if repo.starts_with("cvs+") || repo.contains("pserver:") || repo.contains("@cvs.") {
                return Some("Cvs");
            }
            if repo.starts_with("hg+") || repo.contains("hg@") || repo.contains("/hg.") {
                return Some("Hg");
            }
            if repo.starts_with("svn+") || repo.contains("/svn.") {
                return Some("Svn");
            }
            return None;
        }
        None
    }
}


#[derive(Clone, Debug, Deserialize)]
pub struct Cargo {
    pub package: CargoPackage,
    pub bin: Option<Vec<CargoBin>>,
    pub profile: Option<CargoProfiles>,
}

impl Cargo {
    fn into_config(mut self) -> CDResult<Config> {
        let mut deb = self.package.metadata.take().and_then(|m|m.deb)
            .unwrap_or_else(|| CargoDeb::default());
        let (license_file, license_file_skip_lines) = self.take_license_file(deb.license_file.take())?;
        let readme = self.package.readme.take();
        Ok(Config {
            name: self.package.name.clone(),
            license: self.package.license.clone(),
            license_file,
            license_file_skip_lines,
            copyright: deb.copyright.take().ok_or_then(|| {
                Ok(self.package.authors.as_ref().ok_or("Package must have a copyright or authors")?.join(", "))
            })?,
            version: self.version_string(deb.revision),
            homepage: self.package.homepage.clone(),
            documentation: self.package.documentation.clone(),
            repository: self.package.repository.take(),
            description: self.package.description.clone(),
            extended_description: self.extended_description(deb.extended_description.as_ref().map(|s|s.as_ref()), readme.as_ref().map(|s|s.as_ref()))?,
            maintainer: deb.maintainer.take().ok_or_then(|| {
                Ok(self.package.authors.as_ref().and_then(|a|a.get(0))
                    .ok_or("Package must have a maintainer or authors")?.to_owned())
            })?,
            depends: deb.depends.take().unwrap_or("$auto".to_owned()),
            section: deb.section.take(),
            priority: deb.priority.take().unwrap_or("optional".to_owned()),
            architecture: get_arch().to_owned(),
            conf_files: deb.conf_files.map(|x| x.iter().fold(String::new(), |a, b| a + b + "\n")),
            assets: self.take_assets(deb.assets.take(), readme)?,
            maintainer_scripts: deb.maintainer_scripts.map(|s| PathBuf::from(s)),
            features: deb.features.take().unwrap_or(vec![]),
            default_features: deb.default_features.unwrap_or(true),
            strip: self.profile.and_then(|p|p.release).and_then(|r|r.debug).map(|debug|!debug).unwrap_or(true),
        })
    }

    fn extended_description(&self, desc: Option<&str>, readme: Option<&str>) -> CDResult<Vec<String>> {
        Ok(if let Some(desc) = desc {
            desc.split_by_chars(79)
        } else if let Some(readme) = readme {
            file::get_text(readme)
                .map_err(|e| CargoDebError::IoFile(e, readme.to_owned()))?
                .split_by_chars(159)
        } else {
            vec![]
        })
    }

    fn take_license_file(&mut self, license_file: Option<Vec<String>>) -> CDResult<(Option<String>, usize)> {
        if let Some(mut args) = license_file {
            let mut args = args.drain(..);
            let file = args.next();
            let lines = if let Some(lines) = args.next() {
                lines.parse().map_err(|e| CargoDebError::NumParse("invalid number of lines", e))?
            } else {0};
            Ok((file, lines))
        } else {
            Ok((self.package.license_file.take(), 0))
        }
    }

    fn take_assets(&self, assets: Option<Vec<Vec<String>>>, readme: Option<String>) -> CDResult<Vec<Asset>> {
        Ok(if let Some(assets) = assets {
            assets.into_iter().map(|mut v| {
                let mut v = v.drain(..);
                Ok(Asset {
                    source_file: v.next().ok_or("missing path for asset")?,
                    target_path: v.next().ok_or("missing target for asset")?,
                    chmod: u32::from_str_radix(&v.next().ok_or("missing chmod for asset")?, 8)
                        .map_err(|e| CargoDebError::NumParse("unable to parse chmod argument",e))?,
                })
            }).collect::<Result<Vec<_>, CargoDebError>>()?
        } else {
            let mut implied_assets: Vec<_> = self.bin.as_ref().unwrap_or(&vec![])
                .into_iter()
                .filter(|bin| !bin.plugin.unwrap_or(false) && !bin.proc_macro.unwrap_or(false))
                .map(|bin| {
                Asset {
                    source_file: format!("target/release/{}", bin.name),
                    target_path: format!("usr/bin/{}", bin.name),
                    chmod: 0o755,
                }
            }).collect();
            if let Some(readme) = readme {
                let target_path = format!("usr/share/doc/{}/{}", self.package.name, readme);
                implied_assets.push(Asset {
                    source_file: readme,
                    target_path,
                    chmod: 0o644,
                });
            }
            implied_assets
        })
    }

    fn version_string(&self, revision: Option<String>) -> String {
        if let Some(revision) = revision {
            format!("{}-{}", self.package.version, revision)
        } else {
            self.package.version.clone()
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoPackage {
    pub name: String,
    pub authors: Option<Vec<String>>,
    pub license: String,
    pub license_file: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub version: String,
    pub description: String,
    pub readme: Option<String>,
    pub metadata: Option<CargoMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoMetadata {
    pub deb: Option<CargoDeb>
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoProfiles {
    pub release: Option<CargoProfile>
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoProfile {
    pub debug: Option<bool>
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoBin {
    pub name: String,
    pub plugin: Option<bool>,
    pub proc_macro: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Default)]
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
fn current_manifest_path() -> CDResult<String> {
    let output = Command::new("cargo").arg("locate-project")
        .output().map_err(|e| CargoDebError::CommandFailed(e, "cargo"))?;
    if !output.status.success() {
        return Err(CargoDebError::CommandError("cargo", "locate-project".to_owned(), output.stderr));
    }

    #[derive(Deserialize)]
    struct Data { root: String }
    let stdout = String::from_utf8(output.stdout).unwrap();
    let decoded: Data = serde_json::from_str(&stdout).unwrap();
    Ok(decoded.root)
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
