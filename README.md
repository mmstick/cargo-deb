**Build Status:** [![Build Status](https://travis-ci.org/mmstick/cargo-deb.png?branch=master)](https://travis-ci.org/mmstick/cargo-deb)

# Available Keys

This command will obtain all of the information that it needs from the `Cargo.toml` file, so it is necessary to have filled out enough information in the file for the Debian binary package to be created. `Cargo.toml` already features a number of fields that are immediately useful. These fields are `name`, `version`, `license`, `description`, `homepage`, and `repository`. However, as these fields are not enough, you must also define a new table, `#[package.metadata.deb]` that will contain `maintainer`, `copyright`, `license_file`, `depends`, `extended_description`, `section`, `priority`, and `assets`.

# Key Descriptions

- **maintainer**: The person maintaining the Debian packaging
- **copyright**: To whom and when the copyright of the software is granted
- **license_file**: The location of the license and the amount of lines to skip at the top
- **depends**: The runtime dependencies of the project, which are automatically generated with the `$auto` keyword.=
- **extended_description**: An extended description of the project -- the more detailed the better
- **section**: The application category that the software belongs to
- **priority**: Defines if the package is required or optional
- **assets**: Any other files needed by the package and the permissions to assign them
    - The first argument of each asset is the location of that asset in the Rust project.
    - The second argument is where the file will be copied.
        - If is argument ends with **/** it will be inferred that the target is the directory where the file will be copied.
        - Otherwise, it will be inferred that the source argument will be renamed when copied.
    - The third argument is the permissions to assign that file.
 - **features**: List of Cargo features to use when building the package
 - **default-features**: whether to use default crate features in addition to the `features` list (default `true`)

# Running `cargo deb`
Upon running `cargo deb` from the base directory of your Rust project, a Debian package will be saved in the same
directory. If you would like to handle the build process yourself, you can use `cargo deb --no-build` so that the
`cargo-deb` command will not attempt to rebuild your project.

# Cargo Deb Example

```toml
[package.metadata.deb]
maintainer = "Michael Aaron Murphy <mmstickman@gmail.com>"
copyright = "2016, Michael Aaron Murphy <mmstickman@gmail.com>"
license_file = ["LICENSE", "4"]
extended_description = """\
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

# Systemd Manager Example

```toml
[package.metadata.deb]
maintainer = "Michael Aaron Murphy <mmstickman@gmail.com>"
copyright = "2015-2016, Michael Aaron Murphy <mmstickman@gmail.com>"
license_file = ["LICENSE", "3"]
depends = "$auto"
extended_description = """\
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
