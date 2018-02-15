#![recursion_limit="128"]

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

```rust,ignore
let (options, warnings) = Config::from_manifest(target)?;

reset_deb_directory(&options)?;
cargo_build(&options, target, verbose)?;
strip_binaries(&options, target)?;

let bin_path = generate_debian_binary_file(&options)?;
let mut deb_contents = vec![];
deb_contents.push(bin_path);

let system_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?.as_secs();
// Initailize the contents of the data archive (files that go into the filesystem).
let (data_archive, asset_hashes) = data::generate_archive(&options, system_time)?;
let data_base_path = options.path_in_deb("data.tar");

// Initialize the contents of the control archive (metadata for the package manager).
let control_archive = control::generate_archive(&options, system_time, asset_hashes)?;
let control_base_path = options.path_in_deb("control.tar");

// Order is important for Debian
deb_contents.push(compress::gz(&control_archive, &control_base_path)?);
deb_contents.push(compress::xz_or_gz(&data_archive, &data_base_path)?);

let generated = generate_deb(&options, &deb_contents)?;
```
*/

extern crate toml;
extern crate tar;
#[cfg(feature = "lzma")]
extern crate xz2;
extern crate zopfli;
extern crate md5;
extern crate file;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate getopts;
extern crate glob;

pub mod compress;
pub mod control;
pub mod data;
mod manifest;
mod dependencies;
mod try;
mod wordsplit;
mod error;
mod archive;
mod config;

use std::fs;
use std::path::{Path, PathBuf};
use std::io::{self, Write};
use std::process::Command;
use std::os::unix::fs::OpenOptionsExt;
pub use error::*;

pub use manifest::Config;

const TAR_REJECTS_CUR_DIR: bool = true;

/// Run `dpkg` to install `deb` archive at the given path
pub fn install_deb(path: &Path) -> CDResult<()> {
    let status = Command::new("sudo").arg("dpkg").arg("-i").arg(path)
        .status()?;
    if !status.success() {
        Err(CargoDebError::InstallFailed)?;
    }
    Ok(())
}

/// Uses the ar program to create the final Debian package, at least until a native ar implementation is implemented.
pub fn generate_deb(config: &Config, contents: &[PathBuf]) -> CDResult<PathBuf> {
    let out_relpath = format!("{}_{}_{}.deb", config.name, config.version, config.architecture);
    let out_abspath = config.path_in_deb(&out_relpath);
    {
        let deb_dir = out_abspath.parent().ok_or("invalid dir")?;

        let _ = fs::remove_file(&out_abspath); // Remove it if it exists
        let mut cmd = Command::new("ar");
        cmd.current_dir(&deb_dir).arg("r").arg(out_relpath);
        for path in contents {
            cmd.arg(&path.strip_prefix(&deb_dir).map_err(|_|"invalid path")?);
        }

        let output = cmd.output()
            .map_err(|e| CargoDebError::CommandFailed(e, "ar"))?;
        if !output.status.success() {
            return Err(CargoDebError::CommandError("ar", out_abspath.display().to_string(), output.stderr));
        }
    }
    Ok(out_abspath)
}

/// Creates the debian-binary file that will be added to the final ar archive.
pub fn generate_debian_binary_file(options: &Config) -> io::Result<PathBuf> {
    let bin_path = options.path_in_deb("debian-binary");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o644)
        .open(&bin_path)?;
    file.write(b"2.0\n")?;
    Ok(bin_path)
}

/// Removes the target/debian directory so that we can start fresh.
pub fn reset_deb_directory(options: &Config) -> io::Result<()> {
    let deb_dir = options.deb_dir();
    let _ = fs::remove_dir_all(&deb_dir);
    fs::create_dir_all(deb_dir)
}

/// Builds a release binary with `cargo build --release`
pub fn cargo_build(options: &Config, target: Option<&str>, verbose: bool) -> CDResult<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").args(&["--release", "--all"]);

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

    let status = cmd.status().map_err(|e| CargoDebError::CommandFailed(e, "cargo"))?;
    if !status.success() {
        Err(CargoDebError::BuildFailed)?;
    }
    Ok(())
}

/// Strips the binary that was created with cargo
pub fn strip_binaries(options: &Config, target: Option<&str>) -> CDResult<()> {
    let mut cargo_config = None;
    let strip_tmp;
    let mut strip_cmd = "strip";

    if let Some(target) = target {
        cargo_config = options.cargo_config()?;
        if let Some(ref conf) = cargo_config {
            if let Some(cmd) = conf.strip_command(target) {
                strip_tmp = cmd;
                strip_cmd = &strip_tmp;
            }
        }
    }

    for name in options.binaries() {
        Command::new(strip_cmd)
            .arg("--strip-unneeded")
            .arg(name)
            .status()
            .and_then(|s| if s.success() {
                Ok(())
            } else {
                Err(io::Error::new(io::ErrorKind::Other, format!("{}",s)))
            })
            .map_err(|err| {
                if let Some(target) = target {
                    let conf_path = cargo_config.as_ref().map(|c|c.path()).unwrap_or(Path::new(".cargo/config"));
                    CargoDebError::StripFailed(name.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] strip = \"{}\" in {}", strip_cmd, err, target, strip_cmd, conf_path.display()))
                } else {
                    CargoDebError::CommandFailed(err, "strip")
                }
            })?;
    }
    Ok(())
}
