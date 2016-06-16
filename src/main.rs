extern crate libc;
extern crate rustc_serialize;
extern crate toml;

mod config;

use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use config::Config;

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
    generate_deb(&options);
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
    let mut control = fs::OpenOptions::new().create(true).write(true).open("debian/DEBIAN/control").unwrap();
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

/// Creates a debian directory and copies the files that are needed by the package.
fn copy_files(assets: &[Vec<String>]) {
    fs::create_dir_all("debian/DEBIAN").expect("cargo-deb: unable to create the 'debian/DEBIAN' directory");
    // Copy each of the assets into the debian directory listed in the assets parameter
    for asset in assets {
        // Obtain the target directory of the current asset.
        let mut target = String::from("debian/") + asset.get(1).expect("cargo-deb: missing target directory").as_str();
        // Create the target directory needed by the current asset.
        create_directory(&target);
        // Obtain a reference to the source argument.
        let source = &asset[0];
        // Append the file name to the target directory path.
        target = target.clone() + "/" + Path::new(source).file_name().unwrap().to_str().unwrap();
        // Attempt to copy the file from the source path to the target path.
        match copy_file(source.as_str(), target.as_str(), &asset) {
            Some(CopyError::CopyFailed) => panic!("cargo-deb: unable to copy {} to {}", &source, &target),
            Some(CopyError::MissingChmod) => panic!("cargo-deb: chmod argument is missing from asset: {:?}", asset),
            Some(CopyError::ChmodInvalid(chmod)) => panic!("cargo-deb: chmod argument is invalid: {}", chmod),
            Some(CopyError::ChmodError(chmod)) => panic!("cargo-deb: chmod failed: {}", chmod),
            _ => ()
        }
    }
}

/// Attempt to create the directory neede by the target.
fn create_directory(target: &str) {
    fs::create_dir_all(target).ok()
        .unwrap_or_else(|| panic!("cargo-deb: unable to create the {:?} directory", target));
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
                            let status = unsafe { libc::chmod(CString::new(target).unwrap().as_ptr(), chmod) };
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
