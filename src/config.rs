use error::*;
use std::path::{Path, PathBuf};
use std::env;
use toml;
use file;
use std::borrow::Cow;

pub struct CargoConfig {
    path: PathBuf,
    config: toml::Value,
}

impl CargoConfig {
    pub fn new<P: AsRef<Path>>(project_path: P) -> CDResult<Option<Self>> {
        let mut project_path = project_path.as_ref();
        loop {
            if let Some(conf) = Self::try_parse(project_path)? {
                return Ok(Some(conf));
            }
            if let Some(ref parent) = project_path.parent() {
                project_path = parent;
            } else {
                break;
            }
        }
        if let Some(home) = env::home_dir() {
            if let Some(conf) = Self::try_parse(&home)? {
                return Ok(Some(conf));
            }
        }
        if let Some(conf) = Self::try_parse("/etc")? {
            return Ok(Some(conf));
        }
        Ok(None)
    }

    fn try_parse<P: AsRef<Path>>(path: P) -> CDResult<Option<Self>> {
        let path = path.as_ref().join(".cargo/config");
        if !path.exists() {
            return Ok(None);
        }
        let config = toml::from_str(&file::get_text(&path)?)?;
        Ok(Some(CargoConfig {
            path,
            config,
        }))
    }

    fn target_conf(&self, target_triple: &str) -> Option<&toml::value::Table> {
        if let Some(target) = self.config.get("target").and_then(|t|t.as_table()) {
            return target.get(target_triple).and_then(|t|t.as_table());
        }
        None
    }

    pub fn strip_command(&self, target_triple: &str) -> Option<Cow<str>> {
        if let Some(target) = self.target_conf(target_triple) {
            if let Some(strip) = target.get("strip").and_then(|s|s.as_str()) {
                return Some(Cow::Borrowed(strip));
            }
        }
        if let Some(linker) = self.linker_command(target_triple) {
            if linker.contains('/') {
                let strip_path = Path::new(linker).with_file_name("strip");
                if strip_path.exists() {
                    return Some(Cow::Owned((*strip_path.to_string_lossy()).to_owned()));
                }
            }
        }
        None
    }

    pub fn path(&self) -> &Path {
        return &self.path;
    }

    fn linker_command(&self, target_triple: &str) -> Option<&str> {
        if let Some(target) = self.target_conf(target_triple) {
            return target.get("linker").and_then(|l|l.as_str());
        }
        None
    }
}

#[test]
fn load_conf() {
    if let Some(conf) = CargoConfig::new("/tmp/cargo-deb-test-whatever").unwrap() {
        let _ = conf.strip_command("i686-unknown-dragonfly").unwrap();
    }
}
