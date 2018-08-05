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

```rust,ignore
let listener = &mut listener::StdErrListener {verbose}; // prints warnings
let options = Config::from_manifest(Path::new("Cargo.toml"), target, listener)?;

reset_deb_directory(&options)?;
cargo_build(&options, target, &[], verbose)?;
strip_binaries(&options, target, listener)?;

let bin_path = generate_debian_binary_file(&options)?;
let mut deb_contents = vec![];
deb_contents.push(bin_path);

let system_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?.as_secs();
// Initailize the contents of the data archive (files that go into the filesystem).
let (data_archive, asset_hashes) = data::generate_archive(&options, system_time, listener)?;
let data_base_path = options.path_in_deb("data.tar");

// Initialize the contents of the control archive (metadata for the package manager).
let control_archive = control::generate_archive(&options, system_time, asset_hashes, listener)?;
let control_base_path = options.path_in_deb("control.tar");

// Order is important for Debian
deb_contents.push(compress::gz(&control_archive, &control_base_path)?);
deb_contents.push(compress::xz_or_gz(&data_archive, &data_base_path)?);

let generated = generate_deb(&options, &deb_contents)?;
```
*/

extern crate file;
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
mod try;
mod wordsplit;
mod error;
mod archive;
mod config;
mod pathbytes;
pub mod listener;
use listener::Listener;

use ar::Builder;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{self, Write};
use std::process::Command;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use pathbytes::*;
pub use error::*;

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

/// Uses the ar program to create the final Debian package, at least until a native ar implementation is implemented.
pub fn generate_deb(config: &Config, contents: &[PathBuf]) -> CDResult<PathBuf> {
    let out_relpath = format!("{}_{}_{}.deb", config.name, config.version, config.architecture);
    let out_abspath = config.path_in_deb(&out_relpath);

    {
        let deb_dir = out_abspath.parent().ok_or("invalid dir")?;

        let mut ar_builder = Builder::new(File::create(&out_abspath)?);

        for path in contents {
            let dest_path = path.strip_prefix(&deb_dir).map_err(|_| "invalid path")?;
            let mut file = File::open(&path)?;
            ar_builder.append_file(&dest_path.as_unix_path(), &mut file)?;
        }
    }
    Ok(out_abspath)
}

/// Creates the debian-binary file that will be added to the final ar archive.
pub fn generate_debian_binary_file(options: &Config) -> io::Result<PathBuf> {
    let bin_path = options.path_in_deb("debian-binary");
    let mut opts = fs::OpenOptions::new();
    opts.create(true)
        .write(true)
        .truncate(true);
    #[cfg(unix)]
    {
        opts.mode(0o644);
    }
    let mut file = opts.open(&bin_path)?;
    file.write_all(b"2.0\n")?;
    Ok(bin_path)
}

/// Removes the target/debian directory so that we can start fresh.
pub fn reset_deb_directory(options: &Config) -> io::Result<()> {
    let deb_dir = options.deb_dir();
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
pub fn strip_binaries(options: &Config, target: Option<&str>, listener: &mut Listener) -> CDResult<()> {
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

    for path in options.built_binaries().into_iter().filter_map(|a| a.path()) {
        Command::new(strip_cmd)
            .arg("--strip-unneeded")
            .arg(path)
            .status()
            .and_then(|s| if s.success() {
                Ok(())
            } else {
                Err(io::Error::new(io::ErrorKind::Other, format!("{}",s)))
            })
            .map_err(|err| {
                if let Some(target) = target {
                    let conf_path = cargo_config.as_ref().map(|c|c.path()).unwrap_or(Path::new(".cargo/config"));
                    CargoDebError::StripFailed(path.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] strip = \"{}\" in {}", strip_cmd, err, target, strip_cmd, conf_path.display()))
                } else {
                    CargoDebError::CommandFailed(err, "strip")
                }
            })?;
        listener.info(format!("Stripped '{}'", path.display()));
    }
    Ok(())
}
