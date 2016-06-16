extern crate libc;
extern crate rustc_serialize;
extern crate toml;
extern crate walkdir;

mod config;

use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use config::Config;
use walkdir::WalkDir;

enum CopyError {
    CopyFailed,
    MissingChmod,
    ChmodInvalid(String),
    ChmodError(u32)
}

fn main() {
    let options = Config::new();
    cargo_build();
    copy_files(&options.assets);
    generate_control(&options);
    generate_copyright(&options);
    set_directory_permissions();
    generate_deb(&options);
}

// Rust creates directories with 775 by default. This changes them to the correct permissions, 755.
fn set_directory_permissions() {
    for entry in WalkDir::new("debian").into_iter().map(|entry| entry.unwrap()) {
        if entry.metadata().unwrap().is_dir() {
            let c_string = CString::new(entry.path().to_str().unwrap()).unwrap();
            let status = unsafe { libc::chmod(c_string.as_ptr(), u32::from_str_radix("755", 8).unwrap()) };
            if status < 0 { panic!("cargo-deb: chmod error occurred changing directory permissions"); }
        }
    }
}

/// Attempts to generate a Debian package
fn generate_deb(options: &Config) {
    // fakeroot dpkg-deb --build debian "package-name_version_architecture.deb"
    let package_name = options.name.clone() + "_" + options.version.as_str() + "_" + options.architecture.as_str() + ".deb";
    Command::new("fakeroot").arg("dpkg-deb").arg("--build").arg("debian").arg(&package_name).status().
        expect("cargo-deb: failed to generate Debian package");
}

/// Generates the debian/control file needed by the package.
fn generate_control(options: &Config) {
    let mut control = fs::OpenOptions::new().create(true).write(true).open("debian/DEBIAN/control")
        .expect("cargo-deb: could not create debian/DEBIAN/control");
    control.write(b"Package: ").unwrap();
    control.write(options.name.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Version: ").unwrap();
    control.write(options.version.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Section: ").unwrap();
    control.write(options.section.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Priority: ").unwrap();
    control.write(options.priority.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Standards-Version: 3.9.4\n").unwrap();
    control.write(b"Maintainer: ").unwrap();
    control.write(options.maintainer.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Architecture: ").unwrap();
    control.write(options.architecture.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Depends: ").unwrap();
    control.write(options.depends.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Description: ").unwrap();
    control.write(options.description.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
}

fn generate_copyright(options: &Config) {
    let directory = PathBuf::from("debian/usr/share/doc/").join(options.name.clone());
    fs::create_dir_all(&directory)
        .expect("cargo-deb: unable to create `debian/usr/share/doc/<package>/`");
    let mut control = fs::OpenOptions::new().create(true).write(true).open(&directory.join("copyright"))
        .expect("cargo-deb: could not create debian/DEBIAN/copyright");
    control.write(b"Format: http://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n").unwrap();
    control.write(b"Upstream-Name: ").unwrap();
    control.write(options.name.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Source: ").unwrap();
    control.write(options.repository.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"Copyright: ").unwrap();
    control.write(options.copyright.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    control.write(b"License: ").unwrap();
    control.write(options.license.as_bytes()).unwrap();
    control.write(&[b'\n']).unwrap();
    options.license_file.get(0)
        .map_or_else(|| panic!("cargo-deb: missing license file argument"), |path| {
            let lines_to_skip = options.license_file.get(1).map_or(0, |x| x.parse::<usize>().unwrap_or(0));
            let mut file = fs::File::open(path).expect("cargo-deb: license file not found");
            let mut license_string = String::with_capacity(file.metadata().map(|x| x.len()).unwrap_or(0) as usize);
            file.read_to_string(&mut license_string).expect("cargo-deb: error reading license file");
            for line in license_string.lines().skip(lines_to_skip) {
                let line = line.trim();
                if line.is_empty() {
                    control.write(b".\n").unwrap();
                } else {
                    control.write(line.as_bytes()).unwrap();
                    control.write(&[b'\n']).unwrap();
                }
            }
        });
    let c_string = CString::new(directory.join("copyright").to_str().unwrap()).unwrap();
    let status = unsafe { libc::chmod(c_string.as_ptr(), u32::from_str_radix("644", 8).unwrap()) };
    if status < 0 { panic!("cargo-deb: chmod error occurred in creating copyright file"); }
}

/// Creates a debian directory and copies the files that are needed by the package.
fn copy_files(assets: &[Vec<String>]) {
    fs::create_dir_all("debian/DEBIAN").expect("cargo-deb: unable to create the 'debian/DEBIAN' directory");
    // Copy each of the assets into the debian directory listed in the assets parameter
    for asset in assets {
        // Obtain the target directory of the current asset.
        let mut target = asset.get(1).cloned().expect("cargo-deb: missing target directory");
        target = if target.starts_with('/') {
            String::from("debian") + target.as_str()
        } else {
            String::from("debian/") + target.as_str()
        };
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
            Some(CopyError::CopyFailed) => panic!("cargo-deb: unable to copy {} to {}", &source, &target),
            Some(CopyError::MissingChmod) => panic!("cargo-deb: chmod argument is missing from asset: {:?}", asset),
            Some(CopyError::ChmodInvalid(chmod)) => panic!("cargo-deb: chmod argument is invalid: {}", chmod),
            Some(CopyError::ChmodError(chmod)) => panic!("cargo-deb: chmod failed: {}", chmod),
            _ => ()
        }
    }
}

/// Attempt to create the directory neede by the target.
fn create_directory(target: &str, is_dir: bool) {
    if is_dir {
        fs::create_dir_all(target).ok()
            .unwrap_or_else(|| panic!("cargo-deb: unable to create the {:?} directory", target));
    } else {
        let parent = Path::new(target).parent().unwrap();
        fs::create_dir_all(parent).ok()
            .unwrap_or_else(|| panic!("cargo-deb: unable to create the {:?} directory", target));
    }
}

/// Attempt to copy the source file to the target path.
fn copy_file(source: &str, target: &str, asset: &[String]) -> Option<CopyError> {
    fs::copy(source, target).ok()
        // If the file could not be copied, return the `CopyFailed` error
        .map_or(Some(CopyError::CopyFailed), |_| {
            // Attempt to collect the chmod argument, which is the third argument.
            asset.get(2)
                // If the chmod argument is missing, return `Some(CopyError::MissingChmod)`
                .map_or(Some(CopyError::MissingChmod), |chmod| {
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
fn cargo_build() {
    Command::new("cargo").arg("build").arg("--release").status().expect("cargo-deb: failed to build project");
}
