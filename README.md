# Debian packages from Cargo projects [![Build Status](https://travis-ci.org/mmstick/cargo-deb.svg?branch=master)](https://travis-ci.org/mmstick/cargo-deb)

This is a [Cargo](https://doc.rust-lang.org/cargo/) helper command which automatically creates binary [Debian packages](https://www.debian.org/doc/debian-policy/ch-binary.html) (`.deb`) from Cargo projects.

## Installation

```sh
cargo install cargo-deb
```

Requires Rust 1.31+, and optionally `dpkg`, `ldd` and `liblzma-dev`.

## Usage

```sh
cargo deb
```

Upon running `cargo deb` from the base directory of your Rust project, the Debian package will be created in `target/debian/<project_name>_<version>_<arch>.deb` (or you can change the location with the `--output` option). This package can be installed with `dpkg -i target/debian/*.deb`.

Debug symbols are stripped from the main binary by default, unless `[profile.release] debug = true` is set in `Cargo.toml`. If `cargo deb --separate-debug-symbols` is run, the debug symbols will be packaged as a separate file installed at `/usr/lib/debug/<path-to-binary>.debug`.

`cargo deb --install` builds and installs the project system-wide.

## Configuration

No configuration is necessary to make a basic package from a Cargo project with a binary. This command obtains basic information it needs from [the `Cargo.toml` file](https://doc.rust-lang.org/cargo/reference/manifest.html). It uses Cargo fields: `name`, `version`, `license`, `license-file`, `description`, `readme`, `homepage`, and `repository`.

For a more complete Debian package, you may also define a new table, `[package.metadata.deb]` that contains `maintainer`, `copyright`, `license-file`, `changelog`, `depends`, `conflicts`, `breaks`, `replaces`, `provides`, `extended-description`, `section`, `priority`, and `assets`.

### `[package.metadata.deb]` options

Everything is optional:

- **maintainer**: The person maintaining the Debian packaging. If not present, the first author is used.
- **copyright**: To whom and when the copyright of the software is granted. If not present, the list of authors is used.
- **license-file**: The location of the license and the amount of lines to skip at the top. If not present, package-level `license-file` is used.
- **depends**: The runtime [dependencies](https://www.debian.org/doc/debian-policy/ch-relationships.html) of the project, which are automatically generated with the `$auto` keyword.
- **conflicts**, **breaks**, **replaces**, **provides** — [package transition](https://wiki.debian.org/PackageTransition) control.
- **extended-description**: An extended description of the project — the more detailed the better. Package's `readme` file is used as a fallback.
- **revision**: Version of the Debian package (when the package is updated more often than the project).
- **section**: The [application category](https://packages.debian.org/stretch/) that the software belongs to.
- **priority**: Defines if the package is `required` or `optional`.
- **assets**: Files to be included in the package and the permissions to assign them. If assets are not specified, then defaults are taken from binaries explicitly listed in `[[bin]]` (copied to `/usr/bin/`) and package `readme` (copied to `usr/share/doc/…`).
    1. The first argument of each asset is the location of that asset in the Rust project. Glob patterns are allowed. You can use `target/release/` in asset paths, even if Cargo is configured to cross-compile or use custom `CARGO_TARGET_DIR`. The target dir paths will be automatically corrected.
    2. The second argument is where the file will be copied.
        - If is argument ends with `/` it will be inferred that the target is the directory where the file will be copied.
        - Otherwise, it will be inferred that the source argument will be renamed when copied.
    3. The third argument is the permissions (octal string) to assign that file.
 - **maintainer-scripts** - directory containing `preinst`, `postinst`, `prerm`, or `postrm` [scripts](https://www.debian.org/doc/debian-policy/ch-maintainerscripts.html).
 - **conf-files** - [List of configuration files](https://www.debian.org/doc/manuals/maint-guide/dother.en.html#conffiles) that the package management system will not overwrite when the package is upgraded.
 - **changelog**: Path to Debian-formatted [changelog file](https://www.debian.org/doc/manuals/maint-guide/dreq.en.html#changelog).
 - **features**: List of [Cargo features](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) to use when building the package.
 - **default-features**: whether to use default crate features in addition to the `features` list (default `true`).

### Example of custom `Cargo.toml` additions

```toml
[package.metadata.deb]
maintainer = "Michael Aaron Murphy <mmstickman@gmail.com>"
copyright = "2017, Michael Aaron Murphy <mmstickman@gmail.com>"
license-file = ["LICENSE", "4"]
extended-description = """\
A simple subcommand for the Cargo package manager for \
building Debian packages from Rust projects."""
depends = "$auto"
section = "utility"
priority = "optional"
assets = [
    ["target/release/cargo-deb", "usr/bin/", "755"],
    ["README.md", "usr/share/doc/cargo-deb/README", "644"],
]
```

Systemd Manager:

```toml
[package.metadata.deb]
maintainer = "Michael Aaron Murphy <mmstickman@gmail.com>"
copyright = "2015-2016, Michael Aaron Murphy <mmstickman@gmail.com>"
license-file = ["LICENSE", "3"]
depends = "$auto, systemd"
extended-description = """\
Written safely in Rust, this systemd manager provides a simple GTK3 GUI interface \
that allows you to enable/disable/start/stop services, monitor service logs, and \
edit unit files without ever using the terminal."""
section = "admin"
priority = "optional"
assets = [
    ["assets/org.freedesktop.policykit.systemd-manager.policy", "usr/share/polkit-1/actions/", "644"],
    ["assets/systemd-manager.desktop", "usr/share/applications/", "644"],
    ["assets/systemd-manager-pkexec", "usr/bin/", "755"],
    ["target/release/systemd-manager", "usr/bin/", "755"]
]
```

## Advanced usage

### `[package.metadata.deb.variants.$name]`

There can be multiple variants of the metadata in one `Cargo.toml` file. `--variant=name` selects the variant to use. Options set in a variant override `[package.metadata.deb]` options.

### Cross-compilation

`cargo deb` supports a `--target` flag, which takes [Rust target triple](https://forge.rust-lang.org/platform-support.html). See `rustc --print target-list` for the list of supported values.

Cross-compilation can be run from any host, including macOS and Windows, provided that Debian-compatible linker and system libraries are available to Rust. The target has to be [installed for Rust](https://github.com/rust-lang-nursery/rustup.rs#cross-compilation) (e.g. `rustup target add i686-unknown-linux-gnu`) and has to be [installed for the host system (e.g. Debian)](https://wiki.debian.org/ToolChain/Cross) (e.g. `apt-get install libc6-dev-i386`). Note that Rust's and [Debian's architecture names](https://www.debian.org/ports/) are different.

```sh
cargo deb --target=i686-unknown-linux-gnu
```

Cross-compiled archives are saved in `target/<target triple>/debian/*.deb`. The actual archive path is printed on success.

In `.cargo/config` you can add `[target.<target triple>] strip = { path = "…" } objcopy = { path = "…" }` to specify a path to the architecture-specific `strip` and `objcopy` commands, or use `--no-strip`.

### Separate debug info

    cargo deb --separate-debug-symbols

Removes debug symbols from executables and places them as separate files in `/usr/lib/debug`.

### Custom build flags

If you would like to handle the build process yourself, you can use `cargo deb --no-build` so that the `cargo-deb` command will not attempt to rebuild your project.

    cargo deb -- <cargo build flags>

Flags after `--` are passed to `cargo build`, so you can use options such as `-Z`, `--frozen`, and `--locked`. Please use that only for features that `cargo-deb` doesn't support natively.

### Workspaces

Workspaces are not fully supported yet. [Please leave feedback if you're interested in workspace support](https://github.com/mmstick/cargo-deb/issues/49).

It's possible to build a project in another directory with `cargo deb --manifest-path=<path/to/Cargo.toml>`.
