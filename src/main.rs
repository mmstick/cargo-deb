extern crate libc;
extern crate rustc_serialize;
extern crate toml;
extern crate walkdir;

mod config;
mod try;
mod wordsplit;

use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use config::Config;
use try::{failed, Try};
use walkdir::WalkDir;

enum CopyError {
    CopyFailed,
    ChmodMissing,
    ChmodInvalid(String),
    ChmodError(u32)
}

fn main() {
    let options = Config::new();
    if !std::env::args().any(|x| x.as_str() == "--no-build") { cargo_build(options.name.as_str()); }
    copy_files(&options.assets);
    generate_control(&options);
    generate_copyright(&options);
    set_directory_permissions();
    generate_deb(&options);
}

// Rust creates directories with 775 by default. This changes them to the correct permissions, 755.
fn set_directory_permissions() {
    for entry in WalkDir::new("target/debian").into_iter().map(|entry| entry.unwrap()) {
        if entry.metadata().unwrap().is_dir() {
            let c_string = CString::new(entry.path().to_str().unwrap()).unwrap();
            let status = unsafe { libc::chmod(c_string.as_ptr(), u32::from_str_radix("755", 8).unwrap()) };
            if status < 0 { failed("cargo-deb: chmod error occurred changing directory permissions"); }
        }
    }
}

/// Attempts to generate a Debian package
fn generate_deb(options: &Config) {
    // fakeroot dpkg-deb --build debian "package-name_version_architecture.deb"
    let package_name = options.name.clone() + "_" + options.version.as_str() + "_" + options.architecture.as_str() + ".deb";
    Command::new("fakeroot").arg("dpkg-deb").arg("--build").arg("target/debian").arg(&package_name).status().
        try("cargo-deb: failed to generate Debian package");
}

/// Generates the debian/control file needed by the package.
fn generate_control(options: &Config) {
    // Create and return the handle to the control file with write access.
    let mut control = fs::OpenOptions::new().create(true).write(true).truncate(true).open("target/debian/DEBIAN/control")
        .try("cargo-deb: could not create target/debian/DEBIAN/control");
    // Write all of the lines required by the control file.
    write!(&mut control, "Package: {}\n", options.name).unwrap();
    write!(&mut control, "Version: {}\n", options.version).unwrap();
    write!(&mut control, "Section: {}\n", options.section).unwrap();
    write!(&mut control, "Priority: {}\n", options.priority).unwrap();
    control.write(b"Standards-Version: 3.9.4\n").unwrap();
    write!(&mut control, "Maintainer: {}\n", options.maintainer).unwrap();
    write!(&mut control, "Architecture: {}\n", options.architecture).unwrap();
    write!(&mut control, "Depends: {}\n", options.depends).unwrap();
    write!(&mut control, "Description: {}\n", options.description).unwrap();
    // Write each of the lines that were collected from the extended_description to the file.
    for line in &options.extended_description {
        write!(&mut control, " {}\n", line).unwrap();
    }
}

/// Generates the copyright file needed by the package.
fn generate_copyright(options: &Config) {
    // The directory where the copyright file is stored is named after the name of the package.
    let directory = PathBuf::from("target/debian/usr/share/doc/").join(options.name.clone());
    // Create the directories needed by the copyright file
    fs::create_dir_all(&directory)
        .try("cargo-deb: unable to create `target/debian/usr/share/doc/<package>/`");
    // Open the copyright file for writing
    let mut copyright = fs::OpenOptions::new().create(true).write(true).truncate(true).open(&directory.join("copyright"))
        .try("cargo-deb: could not create target/debian/DEBIAN/copyright");
    // Write the information required by the copyright file to the newly created copyright file.
    copyright.write(b"Format: http://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n").unwrap();
    write!(&mut copyright, "Upstream Name: {}\n", options.name).unwrap();
    write!(&mut copyright, "Source: {}\n", options.repository).unwrap();
    write!(&mut copyright, "Copyright: {}\n", options.copyright).unwrap();
    write!(&mut copyright, "License: {}\n", options.license).unwrap();
    // Attempt to obtain the path of the license file
    options.license_file.get(0)
        // Fail if the path cannot be found and report that the license file argument is missing.
        .map_or_else(|| failed("cargo-deb: missing license file argument"), |path| {
            // Now we need to obtain the amount of lines to skip at the top of the file.
            let lines_to_skip = options.license_file.get(1)
                // If no argument is given, or if the argument is not a number, return 0.
                .map_or(0, |x| x.parse::<usize>().unwrap_or(0));
            // Now we need to attempt to open the file.
            let mut file = fs::File::open(path).try("cargo-deb: license file could not be opened");
            // The capacity of the file can be obtained from the metadata.
            let capacity = file.metadata().map(|x| x.len()).unwrap_or(0) as usize;
            // We are going to store the contents of the license file in a single string with the size of file.
            let mut license_string = String::with_capacity(capacity);
            // Attempt to read the contents of the license file into the license string.
            file.read_to_string(&mut license_string).try("cargo-deb: error reading license file");
            // Skip the first `A` number of lines and then iterate each line after that.
            for line in license_string.lines().skip(lines_to_skip) {
                // If the line is empty, write a dot, else write the line.
                if line.is_empty() {
                    copyright.write(b".\n").unwrap();
                } else {
                    write!(&mut copyright, "{}\n", line.trim()).unwrap();
                }
            }
        });

    // Sets the permissions of the copyright file to `644`.
    let c_string = CString::new(directory.join("copyright").to_str().unwrap()).unwrap();
    let status = unsafe { libc::chmod(c_string.as_ptr(), u32::from_str_radix("644", 8).unwrap()) };
    if status < 0 { failed("cargo-deb: chmod error occurred in creating copyright file"); }
}

/// Creates a debian directory and copies the files that are needed by the package.
fn copy_files(assets: &[Vec<String>]) {
    fs::create_dir_all("target/debian/DEBIAN").try("cargo-deb: unable to create the 'target/debian/DEBIAN' directory");
    // Copy each of the assets into the target/debian directory listed in the assets parameter
    for asset in assets {
        // Obtain the target directory of the current asset.
        let mut target = asset.get(1).map_or_else(|| failed("cargo-deb: missing target directory"), |target| {
            if target.starts_with('/') {
                String::from("target/debian") + target.as_str()
            } else {
                String::from("target/debian/") + target.as_str()
            }
        });

        // Determine if the target is a directory or if the last argument is what to rename the file as.
        let target_is_dir = target.ends_with('/');
        // Create the target directory needed by the current asset.
        create_directory(&target, target_is_dir);
        // Obtain a reference to the source argument.
        let source = &asset[0];
        // Append the file name to the target directory path if the file is not to be renamed.
        if target_is_dir { target = target.clone() + Path::new(source).file_name().unwrap().to_str().unwrap(); }
        // Attempt to copy the file from the source path to the target path.
        match copy_file(source.as_str(), target.as_str(), asset) {
            Some(CopyError::CopyFailed) => {
                failed(format!("cargo-deb: unable to copy {} to {}", &source, &target))
            },
            Some(CopyError::ChmodMissing) => {
                failed(format!("cargo-deb: chmod argument is missing from asset: {:?}", asset))
            },
            Some(CopyError::ChmodInvalid(chmod)) => {
                failed(format!("cargo-deb: chmod argument is invalid: {}", chmod))
            },
            Some(CopyError::ChmodError(chmod)) => {
                failed(format!("cargo-deb: chmod failed: {}", chmod))
            },
            _ => ()
        }
    }
}

/// Attempt to create the directory neede by the target.
fn create_directory(target: &str, is_dir: bool) {
    if is_dir {
        fs::create_dir_all(target).try(&format!("cargo-deb: unable to create the {:?} directory", target));
    } else {
        let parent = Path::new(target).parent().unwrap();
        fs::create_dir_all(parent).try(&format!("cargo-deb: unable to create the {:?} directory", target));
    }
}

/// Attempt to copy the source file to the target path.
fn copy_file(source: &str, target: &str, asset: &[String]) -> Option<CopyError> {
    fs::copy(source, target).ok()
        // If the file could not be copied, return the `CopyFailed` error
        .map_or(Some(CopyError::CopyFailed), |_| {
            // Attempt to collect the chmod argument, which is the third argument.
            asset.get(2)
                // If the chmod argument is missing, return `Some(CopyError::ChmodMissing)`
                .map_or(Some(CopyError::ChmodMissing), |chmod| {
                    // Obtain the octal representation of the chmod argument.
                    u32::from_str_radix(chmod.as_str(), 8).ok()
                        // Return that the value is invalid and return the invalid argument if it is invalid.
                        .map_or(Some(CopyError::ChmodInvalid(chmod.clone())), |chmod| {
                            // Execute the system's chmod command and collect the exit status.
                            let c_string = CString::new(target).unwrap();
                            let status = unsafe { libc::chmod(c_string.as_ptr(), chmod) };
                            // If the exit status is less than zero, return an error, else return `None`.
                            if status < 0 { Some(CopyError::ChmodError(chmod)) } else { None }
                        })
                })
        })

}

/// Builds a release binary with `cargo build --release`
fn cargo_build(name: &str) {
    Command::new("cargo").arg("build").arg("--release").status().try("cargo-deb: failed to build project");
    Command::new("strip").arg("--strip-unneeded")
        .arg(String::from("target/release/") + name)
        .status().try("cargo-deb: could not strip binary");
}
