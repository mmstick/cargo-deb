#![allow(renamed_and_removed_lints)]
use std::io;
use std::num;
use std::path::PathBuf;
use std::time;

quick_error! {
    #[derive(Debug)]
    pub enum CargoDebError {
        Io(err: io::Error) {
            from()
            display("I/O error: {}", err)
            cause(err)
        }
        IoFile(msg: &'static str, err: io::Error, file: PathBuf) {
            display("{}: {}", msg, file.display())
            cause(err)
        }
        CommandFailed(err: io::Error, cmd: &'static str) {
            display("Command {} failed to launch", cmd)
            cause(err)
        }
        CommandError(msg: &'static str, arg: String, reason: Vec<u8>) {
            display("{} ({}): {}", msg, arg, String::from_utf8_lossy(reason))
        }
        Str(msg: &'static str) {
            display("{}", msg)
            from()
        }
        NumParse(msg: &'static str, err: num::ParseIntError) {
            display("{}", msg)
            cause(err)
        }
        InstallFailed {
            display("installation failed, because dpkg -i returned error")
        }
        BuildFailed {
            display("build failed")
        }
        StripFailed(name: PathBuf, reason: String) {
            display("unable to strip binary '{}': {}", name.display(), reason)
        }
        SystemTime(err: time::SystemTimeError) {
            from()
            display("unable to get system time")
            cause(err)
        }
        ParseTOML(err: toml::de::Error) {
            from()
            display("unable to parse Cargo.toml")
            cause(err)
        }
        ParseJSON(err: serde_json::Error) {
            from()
            display("unable to parse `cargo metadata` output")
            cause(err)
        }
        PackageNotFound(path: String, reason: Vec<u8>) {
            display("path '{}' does not belong to a package: {}", path, String::from_utf8_lossy(reason))
        }
        PackageNotFoundInWorkspace(name: String, available: String) {
            display("The workspace doesn't have a package named {}. Available packages are: {}", name, available)
        }
        NoRootFoundInWorkspace(available: String) {
            display("This is a workspace with multiple packages, and there is no single package at the root. Please specify package name with -p. Available packages are: {}", available)
        }
        VariantNotFound(variant: String) {
            display("[package.metadata.deb.variants.{}] not found in Cargo.toml", variant)
        }
        GlobPatternError(err: glob::PatternError) {
            from()
            display("unable to parse glob pattern")
            cause(err)
        }
        AssetFileNotFound(path: PathBuf) {
            display("Asset file path does not match any files: {}", path.display())
        }
        AssetGlobError(err: glob::GlobError) {
            from()
            display("unable to iterate asset glob result")
            cause(err)
        }
    }
}

pub type CDResult<T> = Result<T, CargoDebError>;
