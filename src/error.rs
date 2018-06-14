#![allow(renamed_and_removed_lints)]
use std::io;
use std::num;
use std::time;
use std::path::PathBuf;
use toml;
use serde_json;
use glob;

quick_error! {
    #[derive(Debug)]
    pub enum CargoDebError {
        Io(err: io::Error) {
            from()
            description(err.description())
            display("I/O error: {}", err)
            cause(err)
        }
        IoFile(msg: &'static str, err: io::Error, file: PathBuf) {
            description(msg)
            display("{}: {}", msg, file.display())
            cause(err)
        }
        CommandFailed(err: io::Error, cmd: &'static str) {
            description(err.description())
            display("Command {} failed to launch", cmd)
            cause(err)
        }
        CommandError(msg: &'static str, arg: String, reason: Vec<u8>) {
            description(msg)
            display("{} ({}): {}", msg, arg, String::from_utf8_lossy(reason))
        }
        Str(msg: &'static str) {
            from()
            description(msg)
        }
        NumParse(msg: &'static str, err: num::ParseIntError) {
            description(msg)
            cause(err)
        }
        InstallFailed {
            description("dpkg install failed")
        }
        BuildFailed {
            description("build failed")
        }
        StripFailed(name: PathBuf, reason: String) {
            description(reason)
            display("unable to strip binary '{}': {}", name.display(), reason)
        }
        SystemTime(err: time::SystemTimeError) {
            from()
            description("unable to get system time")
            cause(err)
        }
        ParseTOML(err: toml::de::Error) {
            from()
            description(err.description())
            display("unable to parse Cargo.toml")
            cause(err)
        }
        ParseJSON(err: serde_json::Error) {
            from()
            description(err.description())
            display("unable to parse `cargo metadata` output")
            cause(err)
        }
        PackageNotFound(path: String, reason: Vec<u8>) {
            description("unable to find package for the library")
            display("path '{}' does not belong to a package: {}", path, String::from_utf8_lossy(reason))
        }
        VariantNotFound(variant: String) {
            description("unable to find the passed metadata variant")
            display("[package.metadata.deb.variants.{}] not found in Cargo.toml", variant)
        }
        GlobPatternError(err: glob::PatternError) {
            from()
            description(err.description())
            display("unable to parse glob pattern")
            cause(err)
        }
        AssetGlobError(err: glob::GlobError) {
            from()
            description(err.description())
            display("unable to iterate asset glob result")
            cause(err)
        }
    }
}

pub type CDResult<T> = Result<T, CargoDebError>;
