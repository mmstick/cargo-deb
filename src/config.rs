use crate::error::*;
use std::borrow::Cow;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CargoConfig {
    path: PathBuf,
    config: toml::Value,
}

impl CargoConfig {
    #[allow(deprecated)]
    pub fn new<P: AsRef<Path>>(project_path: P) -> CDResult<Option<Self>> {
        let mut project_path = project_path.as_ref();
        loop {
            if let Some(conf) = Self::try_parse(project_path)? {
                return Ok(Some(conf));
            }
            if let Some(parent) = project_path.parent() {
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
        if path.as_ref().join(".cargo/config").exists() {
            let path = path.as_ref().join(".cargo/config");
            if !path.exists() {
                return Ok(None);
            }
            Ok(Some(Self::from_str(&fs::read_to_string(&path)?, path)?))
        } else {
            let path = path.as_ref().join(".cargo/config.toml");
            if !path.exists() {
                return Ok(None);
            }
            Ok(Some(Self::from_str(&fs::read_to_string(&path)?, path)?))
        }
    }

    fn from_str(input: &str, path: PathBuf) -> CDResult<Self> {
        let config = toml::from_str(input)?;
        Ok(CargoConfig { path, config })
    }

    fn target_conf(&self, target_triple: &str) -> Option<&toml::value::Table> {
        if let Some(target) = self.config.get("target").and_then(|t| t.as_table()) {
            return target.get(target_triple).and_then(|t| t.as_table());
        }
        None
    }

    pub fn strip_command(&self, target_triple: &str) -> Option<Cow<'_, str>> {
        if let Some(target) = self.target_conf(target_triple) {
            let strip_config = target.get("strip").and_then(|top| {
                let as_obj = top.get("path").and_then(|s| s.as_str());
                top.as_str().or(as_obj)
            });
            if let Some(strip) = strip_config {
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
        let path = format!("/usr/bin/{}-strip", crate::debian_triple(target_triple));
        if Path::new(&path).exists() {
            return Some(path.into());
        }
        None
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn linker_command(&self, target_triple: &str) -> Option<&str> {
        if let Some(target) = self.target_conf(target_triple) {
            return target.get("linker").and_then(|l| l.as_str());
        }
        None
    }

    pub fn objcopy_command(&self, target_triple: &str) -> Option<Cow<'_, str>> {
        if let Some(target) = self.target_conf(target_triple) {
            let objcopy_config = target.get("objcopy").and_then(|top| {
                let as_obj = top.get("path").and_then(|s| s.as_str());
                top.as_str().or(as_obj)
            });
            if let Some(objcopy) = objcopy_config {
                return Some(Cow::Borrowed(objcopy));
            }
        }
        if let Some(linker) = self.linker_command(target_triple) {
            if linker.contains('/') {
                let objcopy_path = Path::new(linker).with_file_name("objcopy");
                if objcopy_path.exists() {
                    return Some(Cow::Owned((*objcopy_path.to_string_lossy()).to_owned()));
                }
            }
        }
        None
    }
}

#[test]
fn parse_strip() {
    let c = CargoConfig::from_str(r#"
[target.i686-unknown-dragonfly]
linker = "magic-ld"
strip = "magic-strip"

[target.'foo']
strip = { path = "strip2" }
"#, ".".into()).unwrap();

    assert_eq!("magic-strip", c.strip_command("i686-unknown-dragonfly").unwrap());
    assert_eq!("strip2", c.strip_command("foo").unwrap());
    assert_eq!(None, c.strip_command("bar"));
}

#[test]
fn parse_objcopy() {
    let c = CargoConfig::from_str(r#"
[target.i686-unknown-dragonfly]
linker = "magic-ld"
objcopy = "magic-objcopy"

[target.'foo']
objcopy = { path = "objcopy2" }
"#, ".".into()).unwrap();

    assert_eq!("magic-objcopy", c.objcopy_command("i686-unknown-dragonfly").unwrap());
    assert_eq!("objcopy2", c.objcopy_command("foo").unwrap());
    assert_eq!(None, c.objcopy_command("bar"));
}
