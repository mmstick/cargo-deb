extern crate toml;
extern crate tar;
#[cfg(feature = "lzma")]
extern crate lzma;
extern crate zopfli;
extern crate md5;
extern crate file;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate getopts;

mod compress;
mod manifest;
mod control;
mod data;
mod dependencies;
mod try;
mod wordsplit;
mod error;
mod archive;
mod config;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{self, Write};
use std::process::{self, Command};
use std::time;
use std::os::unix::fs::OpenOptionsExt;
use error::*;

use manifest::Config;

const TAR_REJECTS_CUR_DIR: bool = true;

struct CliOptions {
    no_build: bool,
    no_strip: bool,
    verbose: bool,
    quiet: bool,
    install: bool,
    target: Option<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut cli_opts = getopts::Options::new();
    cli_opts.optflag("", "no-build", "Assume project is already built");
    cli_opts.optflag("", "no-strip", "Do not strip debug symbols from the binary");
    cli_opts.optflag("", "install", "Immediately install created package");
    cli_opts.optopt("", "target", "triple", "Target for cross-compilation");
    cli_opts.optflag("q", "quiet", "Don't print warnings");
    cli_opts.optflag("v", "verbose", "Print progress");
    cli_opts.optflag("h", "help", "Print this help menu");

    let matches = match cli_opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(err) => {
            err_exit(&err);
        },
    };
    if matches.opt_present("h") {
        print!("{}", cli_opts.usage("Usage: cargo deb [options]"));
        return;
    }

    match process(CliOptions {
        no_build: matches.opt_present("no-build"),
        no_strip: matches.opt_present("no-strip"),
        quiet: matches.opt_present("quiet"),
        verbose: matches.opt_present("verbose"),
        install: matches.opt_present("install"),
        target: matches.opt_str("target"),
    }) {
        Ok(()) => {},
        Err(err) => {
            err_exit(&err);
        }
    }
}

fn err_cause(err: &std::error::Error, max: usize) {
    if let Some(reason) = err.cause() {
        eprintln!("  because: {}", reason);
        if max > 0 {
            err_cause(reason, max - 1);
        }
    }
}

fn err_exit(err: &std::error::Error) -> ! {
    eprintln!("cargo-deb: {}", err);
    err_cause(err, 3);
    process::exit(1);
}

fn process(CliOptions {target, install, no_build, no_strip, quiet, verbose}: CliOptions) -> CDResult<()> {
    let (options, warnings) = Config::from_manifest(target.as_ref().map(|s|s.as_ref()))?;
    if !quiet {
        for warning in warnings {
            println!("warning: {}", warning);
        }
    }
    remove_leftover_files(&options.deb_dir())?;

    if !no_build {
        cargo_build(&target, &options.features, options.default_features, verbose)?;
    }
    if options.strip && !no_strip {
        strip_binaries(&options, &target)?;
    }

    // Obtain the current time which will be used to stamp the generated files in the archives.
    let system_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?.as_secs();
    let mut deb_contents = vec![];

    let bin_path = options.path_in_deb("debian-binary");
    generate_debian_binary_file(&bin_path)?;
    deb_contents.push(bin_path);

    // The block frees the large data_archive var early
    {
        // Initailize the contents of the data archive (files that go into the filesystem).
        let (data_archive, asset_hashes) = data::generate_archive(&options, system_time)?;
        let data_base_path = options.path_in_deb("data.tar");

        // Initialize the contents of the control archive (metadata for the package manager).
        let control_archive = control::generate_archive(&options, system_time, asset_hashes)?;
        let control_base_path = options.path_in_deb("control.tar");

        // Order is important for Debian
        deb_contents.push(compress::gz(&control_archive, &control_base_path)?);
        deb_contents.push(compress::xz_or_gz(&data_archive, &data_base_path)?);
    }

    let generated = generate_deb(&options, &deb_contents)?;
    if !quiet {
        println!("{}", generated.display());
    }

    if install {
        install_deb(&generated)?;
    }
    Ok(())
}

fn install_deb(path: &Path) -> CDResult<()> {
    let status = Command::new("sudo").arg("dpkg").arg("-i").arg(path)
        .status()?;
    if !status.success() {
        Err(CargoDebError::InstallFailed)?;
    }
    Ok(())
}

/// Uses the ar program to create the final Debian package, at least until a native ar implementation is implemented.
fn generate_deb(config: &Config, contents: &[PathBuf]) -> CDResult<PathBuf> {
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

// Creates the debian-binary file that will be added to the final ar archive.
fn generate_debian_binary_file(path: &Path) -> io::Result<()> {
    let mut file = fs::OpenOptions::new().create(true).write(true)
        .truncate(true).mode(0o644).open(path)?;
    file.write(b"2.0\n")?;
    Ok(())
}

/// Removes the target/debian directory so that we can start fresh.
fn remove_leftover_files(deb_dir: &Path) -> io::Result<()> {
    let _ = fs::remove_dir_all(deb_dir);
    fs::create_dir_all(deb_dir)
}

/// Builds a release binary with `cargo build --release`
fn cargo_build(target: &Option<String>, features: &[String], default_features: bool, verbose: bool) -> CDResult<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--release");

    if verbose {
        cmd.arg("--verbose");
    }
    if let Some(ref target) = *target {
        cmd.arg(format!("--target={}", target));
    }
    if !default_features {
        cmd.arg("--no-default-features");
    }
    if !features.is_empty() {
        cmd.arg(format!("--features={}", features.join(",")));
    }

    let status = cmd.status().map_err(|e| CargoDebError::CommandFailed(e, "cargo"))?;
    if !status.success() {
        Err(CargoDebError::BuildFailed)?;
    }
    Ok(())
}

// Strips the binary that was created with cargo
fn strip_binaries(options: &Config, target: &Option<String>) -> CDResult<()> {
    let mut cargo_config = None;
    let strip_tmp;
    let mut strip_cmd = "strip";

    if let Some(ref target) = *target {
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
                if let Some(ref target) = *target {
                    let conf_path = cargo_config.as_ref().map(|c|c.path()).unwrap_or(Path::new(".cargo/config"));
                    CargoDebError::StripFailed(name.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] strip = \"{}\" in {}", strip_cmd, err, target, strip_cmd, conf_path.display()))
                } else {
                    CargoDebError::CommandFailed(err, "strip")
                }
            })?;
    }
    Ok(())
}
