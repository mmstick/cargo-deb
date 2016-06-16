#![feature(libc)]
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

fn main() {
    let options = Config::new();
    cargo_build();
    copy_files(&options.assets);
    generate_control(&options);
    generate_deb(&options);
}

fn generate_deb(options: &Config) {
    // fakeroot dpkg-deb --build debian "package-name_version_architecture.deb"
    let package_name = options.name.clone() + "_" + options.version.as_str() + "_" + options.architecture.as_str() + ".deb";
    Command::new("fakeroot").arg("dpkg-deb").arg("--build").arg("debian").arg(&package_name).status().
        expect("cargo-deb: failed to generate Debian package");
}

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

fn copy_files(assets: &[Vec<String>]) {
    fs::create_dir_all("debian/DEBIAN").expect("cargo-deb: unable to create the 'debian/DEBIAN' directory");

    for asset in assets {
        let target = asset.get(1).expect("cargo-deb: missing target directory");
        let target = String::from("debian/") + target.as_str();
        fs::create_dir_all(target.clone()).ok()
            .unwrap_or_else(|| panic!("cargo-deb: unable to create the {:?} directory", target));

        // Copy the asset's source to the target path.
        let source = &asset[0];
        let file_name = Path::new(source).file_name().unwrap().to_str().unwrap();
        let target = target.clone() + "/" + file_name;
        fs::copy(source, target.clone()).ok()
            .unwrap_or_else(|| panic!("cargo-deb: unable to copy {} to {}", source, target));

        // Set the permissions of the new source file.
        let source = CString::new(target.as_str()).unwrap();
        let chmod = asset.get(2).unwrap_or_else(|| panic!("cargo-deb: missing chmod in {:?}", asset));
        let chmod = u32::from_str_radix(chmod.as_str(), 8).expect("cargo-deb: chmod value is invalid");
        unsafe {
            if libc::chmod(source.as_ptr(), chmod) < 0 {
                panic!("cargo-deb: error in chmod: {}", chmod);
            }
        }
    }
}

fn cargo_build() {
    Command::new("cargo").arg("build").arg("--release").status().expect("cargo-deb: failed to build project");
}
