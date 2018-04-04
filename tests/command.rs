#![allow(unused_imports)]
extern crate file;
extern crate tempdir;
use std::env;
use std::process::Command;
use std::path::{Path, PathBuf};
use tempdir::TempDir;

#[test]
#[cfg(all(feature = "lzma", target_os = "linux"))]
fn run_cargo_deb_command_on_example_dir() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cmd_path = root.join("target/debug/cargo-deb");
    assert!(cmd_path.exists());
    let output = Command::new(cmd_path)
        .arg(format!("--manifest-path={}", root.join("example/Cargo.toml").display()))
        .output().unwrap();
    assert!(output.status.success());

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len()-1].split(|&c| c==b'\n').last().unwrap();
    let deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert!(deb_path.exists());

    let ardir = TempDir::new("cargo-deb-test").unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", file::get_text(ardir.path().join("debian-binary")).unwrap());
    assert!(ardir.path().join("data.tar.xz").exists());
    assert!(ardir.path().join("control.tar.gz").exists());

    let cdir = TempDir::new("cargo-control-test").unwrap();
    assert!(Command::new("tar")
        .arg("xzf")
        .current_dir(cdir.path())
        .arg(ardir.path().join("control.tar.gz"))
        .status().unwrap().success());

    let control = file::get_text(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example\n"));
    assert!(control.contains("Version: 0.1.0\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = file::get_text(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example/changelog.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("1537684900f6b12358c88a612adf1049  var/lib/example/3.txt\n"));
    assert!(md5sums.contains("e82262a2b9598001688d507f16ea3a61  usr/share/doc/example/copyright\n"));

    let ddir = TempDir::new("cargo-data-test").unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join("data.tar.xz"))
        .status().unwrap().success());

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/3.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example/changelog.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
}

#[test]
#[cfg(all(feature = "lzma"))]
fn run_cargo_deb_command_on_example_dir_with_variant() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cmd_path = root.join("target/debug/cargo-deb");
    assert!(cmd_path.exists());
    let output = Command::new(cmd_path)
        .arg("--variant=debug")
        .arg("--no-strip")
        .arg(format!(
            "--manifest-path={}",
            root.join("example/Cargo.toml").display()
        ))
        .output()
        .unwrap();
    assert!(output.status.success());

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len()-1].split(|&c| c==b'\n').last().unwrap();
    let deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert!(deb_path.exists());

    let ardir = TempDir::new("cargo-deb-test2").unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", file::get_text(ardir.path().join("debian-binary")).unwrap());
    assert!(ardir.path().join("data.tar.xz").exists());
    assert!(ardir.path().join("control.tar.gz").exists());

    let cdir = TempDir::new("cargo-control-test").unwrap();
    assert!(Command::new("tar")
        .arg("xzf")
        .current_dir(cdir.path())
        .arg(ardir.path().join("control.tar.gz"))
        .status().unwrap().success());

    let control = file::get_text(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example-debug\n"), "Control is: {:?}", control);
    assert!(control.contains("Version: 0.1.0\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = file::get_text(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example-debug/changelog.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("835a3c46f2330925774ebf780aa74241  var/lib/example/4.txt\n"));
    assert!(md5sums.contains("9a6e2d2dd978ea60b29260416ee09dbc  usr/share/doc/example-debug/copyright\n"));

    let ddir = TempDir::new("cargo-data-test").unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join("data.tar.xz"))
        .status().unwrap().success());

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/4.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example-debug/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example-debug/changelog.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
}
