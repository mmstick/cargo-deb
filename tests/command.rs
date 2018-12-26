use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());
    assert!(ardir.path().join("data.tar.xz").exists());
    assert!(ardir.path().join("control.tar.gz").exists());

    let cdir = TempDir::new("cargo-control-test").unwrap();
    assert!(Command::new("tar")
        .arg("xzf")
        .current_dir(cdir.path())
        .arg(ardir.path().join("control.tar.gz"))
        .status().unwrap().success());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example\n"));
    assert!(control.contains("Version: 0.1.0\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = fs::read_to_string(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example/changelog.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("1537684900f6b12358c88a612adf1049  var/lib/example/3.txt\n"));
    assert!(md5sums.contains("4176f128e63dbe2f7ba37490bd0368db  usr/share/doc/example/copyright\n"));

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
    // changelog.gz starts with the gzip magic
    assert_eq!(
        &[0x1F, 0x8B],
        &fs::read(ddir.path().join("usr/share/doc/example/changelog.gz")).unwrap()[..2]
    );
}

#[test]
#[cfg(all(feature = "lzma"))]
fn run_cargo_deb_command_on_example_dir_with_variant() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cmd_path = root.join(format!("target/debug/cargo-deb{}", std::env::consts::EXE_SUFFIX));
    assert!(cmd_path.exists());
    let cargo_dir = TempDir::new("cargo-deb-target").unwrap();
    let deb_path = cargo_dir.path().join("test.deb");
    let output = Command::new(cmd_path)
        .env("CARGO_TARGET_DIR", cargo_dir.path()) // otherwise tests overwrite each other
        .arg("--variant=debug")
        .arg("--no-strip")
        .arg(format!("--output={}", deb_path.display()))
        .arg(format!(
            "--manifest-path={}",
            root.join("example/Cargo.toml").display()
        ))
        .output()
        .unwrap();
    if !output.status.success() {
        panic!("Cmd failed: {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len()-1].split(|&c| c==b'\n').last().unwrap();
    let printed_deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert_eq!(printed_deb_path, deb_path);
    assert!(deb_path.exists());

    let ardir = TempDir::new("cargo-deb-test2").unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());
    assert!(ardir.path().join("data.tar.xz").exists());
    assert!(ardir.path().join("control.tar.gz").exists());

    let cdir = TempDir::new("cargo-control-test").unwrap();
    assert!(Command::new("tar")
        .arg("xzf")
        .current_dir(cdir.path())
        .arg(ardir.path().join("control.tar.gz"))
        .status().unwrap().success());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example-debug\n"), "Control is: {:?}", control);
    assert!(control.contains("Version: 0.1.0\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = fs::read_to_string(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example-debug/changelog.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("835a3c46f2330925774ebf780aa74241  var/lib/example/4.txt\n"));
    assert!(md5sums.contains("f4b165c5ea1f9ec1b87abd72845627fd  usr/share/doc/example-debug/copyright\n"));

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
