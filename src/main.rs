use cargo_deb::*;
use std::env;
use std::path::Path;
use std::process;
use std::time;

struct CliOptions {
    no_build: bool,
    no_strip: bool,
    separate_debug_symbols: bool,
    fast: bool,
    verbose: bool,
    quiet: bool,
    install: bool,
    package_name: Option<String>,
    output_path: Option<String>,
    variant: Option<String>,
    target: Option<String>,
    manifest_path: Option<String>,
    cargo_build_flags: Vec<String>,
    deb_version: Option<String>,
    no_release: bool,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut cli_opts = getopts::Options::new();
    cli_opts.optflag("", "no-build", "Assume project is already built");
    cli_opts.optflag("", "no-strip", "Do not strip debug symbols from the binary");
    cli_opts.optflag("", "separate-debug-symbols", "Strip debug symbols into a separate .debug file");
    cli_opts.optflag("", "fast", "Use faster compression, which yields larger archive");
    cli_opts.optflag("", "install", "Immediately install created package");
    cli_opts.optopt("", "target", "Rust target for cross-compilation", "triple");
    cli_opts.optopt("", "variant", "Alternative configuration section to use", "name");
    cli_opts.optopt("", "manifest-path", "Cargo project file location", "./Cargo.toml");
    cli_opts.optopt("p", "package", "Select one of packages belonging to a workspace", "name");
    cli_opts.optopt("o", "output", "Write .deb to this file or directory", "path");
    cli_opts.optflag("q", "quiet", "Don't print warnings");
    cli_opts.optflag("v", "verbose", "Print progress");
    cli_opts.optflag("h", "help", "Print this help menu");
    cli_opts.optflag("", "version", "Show the version of cargo-deb");
    cli_opts.optopt("", "deb-version", "Alternate version string for package", "version");
    cli_opts.optflag("", "no-release", "Used in combination with 'no-build'. Assumes a none release build profile.");


    let matches = match cli_opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(err) => {
            err_exit(&err);
        },
    };
    if matches.opt_present("h") {
        print!("{}", cli_opts.usage("Usage: cargo deb [options] [-- <cargo build flags>]"));
        return;
    }

    if matches.opt_present("version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let install = matches.opt_present("install");
    match process(CliOptions {
        no_build: matches.opt_present("no-build"),
        no_strip: matches.opt_present("no-strip"),
        separate_debug_symbols: matches.opt_present("separate-debug-symbols"),
        quiet: matches.opt_present("quiet"),
        verbose: matches.opt_present("verbose"),
        install,
        // when installing locally it won't be transferred anywhere, so allow faster compression
        fast: install || matches.opt_present("fast"),
        variant: matches.opt_str("variant"),
        target: matches.opt_str("target"),
        output_path: matches.opt_str("output"),
        package_name: matches.opt_str("package"),
        manifest_path: matches.opt_str("manifest-path"),
        deb_version: matches.opt_str("deb-version"),
        no_release: matches.opt_present("no-release"),
        cargo_build_flags: matches.free,
    }) {
        Ok(()) => {},
        Err(err) => {
            err_exit(&err);
        }
    }
}

#[allow(deprecated)]
fn err_cause(err: &dyn std::error::Error, max: usize) {
    if let Some(reason) = err.cause() { // we use cause(), not source()
        eprintln!("  because: {}", reason);
        if max > 0 {
            err_cause(reason, max - 1);
        }
    }
}

fn err_exit(err: &dyn std::error::Error) -> ! {
    eprintln!("cargo-deb: {}", err);
    err_cause(err, 3);
    process::exit(1);
}

fn process(
    CliOptions {
        manifest_path,
        output_path,
        package_name,
        variant,
        target,
        install,
        no_build,
        no_strip,
        separate_debug_symbols,
        quiet,
        fast,
        verbose,
        mut cargo_build_flags,
        deb_version,
        no_release,
    }: CliOptions,
) -> CDResult<()> {
    let target = target.as_deref();
    let variant = variant.as_deref();

    if install || target.is_none() {
        warn_if_not_linux(); // compiling natively for non-linux = nope
    }

    // `cargo deb` invocation passes the `deb` arg through.
    if cargo_build_flags.first().map_or(false, |arg| arg == "deb") {
        cargo_build_flags.remove(0);
    }

    // Listener conditionally prints warnings
    let mut listener_tmp1;
    let mut listener_tmp2;
    let listener: &mut dyn listener::Listener = if quiet {
        listener_tmp1 = listener::NoOpListener;
        &mut listener_tmp1
    } else {
        listener_tmp2 = listener::StdErrListener { verbose };
        &mut listener_tmp2
    };

    let manifest_path = manifest_path.as_ref().map_or("Cargo.toml", |s| s.as_str());
    let mut options = Config::from_manifest(
        Path::new(manifest_path),
        package_name.as_deref(),
        output_path,
        target,
        variant,
        deb_version,
        listener,
        no_release,
    )?;
    reset_deb_temp_directory(&options)?;

    if !no_build {
        cargo_build(&options, target, &cargo_build_flags, verbose)?;
    }

    options.resolve_assets()?;

    crate::data::compress_assets(&mut options, listener)?;

    if (options.strip || separate_debug_symbols) && !no_strip {
        strip_binaries(&mut options, target, listener, separate_debug_symbols)?;
    }

    // Obtain the current time which will be used to stamp the generated files in the archives.
    let system_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?.as_secs();
    let mut deb_contents = DebArchive::new(&options)?;

    deb_contents.add_data("debian-binary", system_time, b"2.0\n")?;

    // Initailize the contents of the data archive (files that go into the filesystem).
    let (data_archive, asset_hashes) = data::generate_archive(&options, system_time, listener)?;
    let original = data_archive.len();

    let listener_tmp = &mut *listener; // reborrow for the closure
    let options = &options;
    let (control_compressed, data_compressed) = rayon::join(move || {
        // The control archive is the metadata for the package manager
        let control_archive = control::generate_archive(options, system_time, asset_hashes, listener_tmp)?;
        compress::xz_or_gz(&control_archive, fast)
    }, move || {
        compress::xz_or_gz(&data_archive, fast)
    });
    let control_compressed = control_compressed?;
    let data_compressed = data_compressed?;

    // Order is important for Debian
    deb_contents.add_data(&format!("control.tar.{}", control_compressed.extension()), system_time, &control_compressed)?;
    drop(control_compressed);
    let compressed = data_compressed.len();
    listener.info(format!(
        "compressed/original ratio {}/{} ({}%)",
        compressed,
        original,
        compressed * 100 / original
    ));
    deb_contents.add_data(&format!("data.tar.{}", data_compressed.extension()), system_time, &data_compressed)?;
    drop(data_compressed);

    let generated = deb_contents.finish()?;
    if !quiet {
        println!("{}", generated.display());
    }

    remove_deb_temp_directory(&options);

    if install {
        install_deb(&generated)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn warn_if_not_linux() {}

#[cfg(not(target_os = "linux"))]
fn warn_if_not_linux() {
    eprintln!("warning: This command is for Linux only, and will not make sense when run on other systems");
}
