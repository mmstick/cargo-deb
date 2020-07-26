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

#[macro_use] extern crate quick_error;
pub mod compress;
pub mod control;
pub mod data;
pub mod listener;
pub mod manifest;
pub use crate::debarchive::DebArchive;
pub use crate::error::*;
pub use crate::manifest::Config;

mod config;
mod debarchive;
mod dependencies;
mod error;
mod ok_or;
mod pathbytes;
mod tararchive;
mod wordsplit;
mod dh_installsystemd;
mod dh_lib;
mod util;

use crate::listener::Listener;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

const TAR_REJECTS_CUR_DIR: bool = true;

/// created by `build.rs`
const DEFAULT_TARGET: &str = include_str!(concat!(env!("OUT_DIR"), "/default_target.rs"));

/// Run `dpkg` to install `deb` archive at the given path
pub fn install_deb(path: &Path) -> CDResult<()> {
    let status = Command::new("sudo").arg("dpkg").arg("-i").arg(path)
        .status()?;
    if !status.success() {
        return Err(CargoDebError::InstallFailed);
    }
    Ok(())
}

/// Creates empty (removes files if needed) target/debian/foo directory so that we can start fresh.
pub fn reset_deb_temp_directory(options: &Config) -> io::Result<()> {
    let deb_dir = options.default_deb_output_dir();
    let deb_temp_dir = options.deb_temp_dir();
    remove_deb_temp_directory(options);
    // For backwards compatibility with previous cargo-deb behavior, also delete .deb from target/debian,
    // but this time only debs from other versions of the same package
    let g = deb_dir.join(DebArchive::filename_glob(options));
    if let Ok(old_files) = glob::glob(g.to_str().expect("utf8 path")) {
        for old_file in old_files {
            if let Ok(old_file) = old_file {
                let _ = fs::remove_file(old_file);
            }
        }
    }
    fs::create_dir_all(deb_temp_dir)
}

/// Removes the target/debian/foo
pub fn remove_deb_temp_directory(options: &Config) {
    let deb_temp_dir = options.deb_temp_dir();
    let _ = fs::remove_dir(&deb_temp_dir);
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
        // Set helpful defaults for cross-compiling
        if env::var_os("PKG_CONFIG_ALLOW_CROSS").is_none() && env::var_os("PKG_CONFIG_PATH").is_none() {
            let pkg_config_path = format!("/usr/lib/{}/pkgconfig", debian_triple(target));
            if Path::new(&pkg_config_path).exists() {
                cmd.env("PKG_CONFIG_ALLOW_CROSS", "1");
                cmd.env("PKG_CONFIG_PATH", pkg_config_path);
            }
        }
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
        return Err(CargoDebError::BuildFailed);
    }
    Ok(())
}

// Maps Rust's blah-unknown-linux-blah to Debian's blah-linux-blah
fn debian_triple(rust_target_triple: &str) -> String {
    let mut p = rust_target_triple.split('-');
    let arch = p.next().unwrap();
    let abi = p.last().unwrap_or("");

    let (darch, dabi) = match (arch, abi) {
        ("i586", _) |
        ("i686", _) => ("i386", "gnu"),
        ("x86_64", _) => ("x86_64", "gnu"),
        ("aarch64", _) => ("aarch64", "gnu"),
        (arm, abi) if arm.starts_with("arm") || arm.starts_with("thumb") => {
            ("arm", if abi.ends_with("hf") {"gnueabihf"} else {"gnueabi"})
        },
        ("mipsel", _) => ("mipsel", "gnu"),
        (risc, _) if risc.starts_with("riscv64") => ("riscv64", "gnu"),
        (arch, abi) => (arch, abi),
    };
    format!("{}-linux-{}", darch, dabi)
}

fn ensure_success(status: ExitStatus) -> io::Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, format!("{}", status)))
    }
}

/// Strips the binary that was created with cargo
pub fn strip_binaries(options: &mut Config, target: Option<&str>, listener: &mut dyn Listener, separate_file: bool) -> CDResult<()> {
    let mut cargo_config = None;
    let objcopy_tmp;
    let strip_tmp;
    let mut objcopy_cmd = "objcopy";
    let mut strip_cmd = "strip";

    if let Some(target) = target {
        cargo_config = options.cargo_config()?;
        if let Some(ref conf) = cargo_config {
            if let Some(cmd) = conf.objcopy_command(target) {
                listener.info(format!("Using '{}' for '{}'", cmd, target));
                objcopy_tmp = cmd;
                objcopy_cmd = &objcopy_tmp;
            }

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
                let conf_path = cargo_config
                    .as_ref()
                    .map(|c| c.path())
                    .unwrap_or_else(|| Path::new(".cargo/config"));

                if separate_file {
                    Command::new(objcopy_cmd)
                        .arg("--only-keep-debug")
                        .arg(path)
                        .arg(&debug_path)
                        .status()
                        .and_then(ensure_success)
                        .map_err(|err| {
                            if let Some(target) = target {
                                CargoDebError::StripFailed(path.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] objcopy = {{ path =\"{}\" }} in {}", objcopy_cmd, err, target, objcopy_cmd, conf_path.display()))
                            } else {
                                CargoDebError::CommandFailed(err, "objcopy")
                            }
                        })?;
                }
                Command::new(strip_cmd)
                   .arg("--strip-unneeded")
                   .arg(path)
                   .status()
                   .and_then(ensure_success)
                   .map_err(|err| {
                        if let Some(target) = target {
                            CargoDebError::StripFailed(path.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] strip = {{ path = \"{}\" }} in {}", strip_cmd, err, target, strip_cmd, conf_path.display()))
                        } else {
                            CargoDebError::CommandFailed(err, "strip")
                        }
                    })?;
                if separate_file {
                    Command::new(objcopy_cmd)
                        .current_dir(
                            debug_path
                                .parent()
                                .expect("Debug source file had no parent path"),
                        )
                        .arg(format!(
                            "--add-gnu-debuglink={}",
                            debug_filename
                                .to_str()
                                .expect("Debug source file had no filename")
                        ))
                        .arg(path)
                        .status()
                        .and_then(ensure_success)
                        .map_err(|err| CargoDebError::CommandFailed(err, "objcopy"))?;
                }
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
