#![recursion_limit = "128"]

/*!

## Making deb packages

If you only want to make some `*.deb` files, and you're not a developer of tools
for Debian packaging, **[see `cargo deb` command usage described in the
README instead](https://github.com/mmstick/cargo-deb#readme)**.

```sh
cargo install cargo-deb
cargo deb # run this in your Cargo project directory
```

## Making tools for making deb packages

The library interface is experimental. See `main.rs` for usage.
*/

extern crate getopts;
extern crate glob;
extern crate md5;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tar;
extern crate ar;
extern crate toml;
#[cfg(feature = "lzma")]
extern crate xz2;
extern crate zopfli;
extern crate cargo_toml;

pub mod compress;
pub mod control;
pub mod data;
mod manifest;
mod dependencies;
mod ok_or;
mod wordsplit;
mod error;
mod tararchive;
mod debarchive;
mod config;
mod pathbytes;
pub mod listener;
use listener::Listener;

use std::fs;
use std::path::Path;
use std::io;
use std::process::Command;
pub use error::*;

pub use debarchive::DebArchive;
pub use manifest::Config;

const TAR_REJECTS_CUR_DIR: bool = true;

/// created by `build.rs`
const DEFAULT_TARGET: &str = include_str!(concat!(env!("OUT_DIR"), "/default_target.rs"));

/// Run `dpkg` to install `deb` archive at the given path
pub fn install_deb(path: &Path) -> CDResult<()> {
    let status = Command::new("sudo").arg("dpkg").arg("-i").arg(path)
        .status()?;
    if !status.success() {
        Err(CargoDebError::InstallFailed)?;
    }
    Ok(())
}

/// Removes the target/debian directory so that we can start fresh.
pub fn reset_deb_directory(options: &Config) -> io::Result<()> {
    let deb_dir = options.deb_temp_dir();
    let _ = fs::remove_dir_all(&deb_dir);
    fs::create_dir_all(deb_dir)
}

/// Builds a release binary with `cargo build --release`
pub fn cargo_build(options: &Config, target: Option<&str>, other_flags: &[String], verbose: bool) -> CDResult<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&options.manifest_dir);
    cmd.arg("build").args(&["--release", "--all"]);

    for flag in other_flags {
        cmd.arg(flag);
    }

    if verbose {
        cmd.arg("--verbose");
    }
    if let Some(target) = target {
        cmd.arg(format!("--target={}", target));
    }
    if !options.default_features {
        cmd.arg("--no-default-features");
    }
    let features = &options.features;
    if !features.is_empty() {
        cmd.arg(format!("--features={}", features.join(",")));
    }

    let status = cmd.status()
        .map_err(|e| CargoDebError::CommandFailed(e, "cargo"))?;
    if !status.success() {
        Err(CargoDebError::BuildFailed)?;
    }
    Ok(())
}

/// Strips the binary that was created with cargo
pub fn strip_binaries(options: &mut Config, target: Option<&str>, listener: &mut Listener, separate_file: bool) -> CDResult<()> {
    let mut cargo_config = None;
    let strip_tmp;
    let mut strip_cmd = "strip";

    if let Some(target) = target {
        cargo_config = options.cargo_config()?;
        if let Some(ref conf) = cargo_config {
            if let Some(cmd) = conf.strip_command(target) {
                listener.info(format!("Using '{}' for '{}'", cmd, target));
                strip_tmp = cmd;
                strip_cmd = &strip_tmp;
            }
        }
    }

    for asset in options.built_binaries() {
        match asset.source.path() {
            Some(path) => {
                // We always strip the symbols to a separate file,  but they will only be included if specified

                // The debug_path and debug_filename should never return None if we have an AssetSource::Path
                let debug_path = asset.source.debug_source().expect("Failed to compute debug source path");
                let debug_filename = debug_path.file_name().expect("Built binary has no filename");

                Command::new("objcopy")
                    .arg("--only-keep-debug")
                    .arg(path)
                    .arg(&debug_path)
                    .status()
                    .and_then(|s| {
                        if s.success() {
                            Command::new(strip_cmd)
                                .arg("--strip-unneeded")
                                .arg(path)
                                .status()
                                .and_then(|s| {
                                    if s.success() {
                                        Command::new("objcopy")
                                            .current_dir(debug_path.parent().expect("Debug source file had no parent path"))
                                            .arg(format!("--add-gnu-debuglink={}", debug_filename.to_str().expect("Debug source file had no filename")))
                                            .arg(path)
                                            .status()
                                            .and_then(|s| {
                                                if s.success() {
                                                    Ok(())
                                                }
                                                else {
                                                    Err(io::Error::new(io::ErrorKind::Other, format!("{}", s)))
                                                }
                                        })
                                    } else {
                                        Err(io::Error::new(io::ErrorKind::Other, format!("{}", s)))
                                    }
                                })
                        } else {
                            Err(io::Error::new(io::ErrorKind::Other, format!("{}", s)))
                        }
                    })
                    .map_err(|err| {
                        if let Some(target) = target {
                            let conf_path = cargo_config
                                .as_ref()
                                .map(|c| c.path())
                                .unwrap_or(Path::new(".cargo/config"));
                            CargoDebError::StripFailed(path.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] strip = \"{}\" in {}", strip_cmd, err, target, strip_cmd, conf_path.display()))
                        } else {
                            CargoDebError::CommandFailed(err, "strip")
                        }
                    })?;
                listener.info(format!("Stripped '{}'", path.display()));
            },
            None => {
                // This is unexpected - emit a warning if we come across it
                listener.warning(format!("Found built asset with non-path source '{:?}'", asset));
            }
        }
    }

    if separate_file {
        // If we want to debug symols included in a separate file, add these files to the debian assets
        options.add_debug_assets();
    }

    Ok(())
}
