use std::process::Command;
use std::fmt::Write;
use itertools::Itertools;
use try::{Try, failed};

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve<S: AsRef<str>>(path: S) -> String {
    let dependencies = {
        let path = path.as_ref();
        let output = Command::new("ldd").arg(path).output().map(|x| x.stdout)
            .try("cargo-deb: failed to launch ldd command");
        String::from_utf8(output).unwrap()
    };

    // Create an iterator of unique dependencies
    let mut dependencies = dependencies.lines()
        // We only want the third field on each line, which contains the filepath of the library.
        .map(|line| line.split_whitespace().nth(2))
        // If the field exists and starts with '/', we have found a filepath.
        .filter(|x| x.is_some() && x.unwrap().chars().next().unwrap() == '/')
        // Obtain the names of the packages.
        .map(|path| get_package_name(path.unwrap()))
        // only collect unique packages.
        .unique();

    // Create a formatted string with the output from ldd.
    let mut output = String::with_capacity(256);
    if let Some(package) = dependencies.next() {
        if let Some(version) = get_version(&package) {
            write!(&mut output, "{} (>= {})", &package, &version).unwrap();
            for package in dependencies {
                write!(&mut output, ", {} (>= {})", &package, &version).unwrap();
            }
        } else {
            failed(format!("Unable to get version of package {}", package));
        }
    }

    output
}

/// Obtains the name of the package that belongs to the file that ldd returned.
fn get_package_name(path: &str) -> String {
    let output = Command::new("dpkg").arg("-S").arg(path).output().ok().map(|x| x.stdout)
        .try("cargo-deb: dpkg command not found. Automatic dependency resolution is only supported on Debian.");
    let package = output.iter().take_while(|&&x| x != b':').cloned().collect::<Vec<u8>>();
    String::from_utf8(package).unwrap()
}

/// Uses apt-cache policy to determine the version of the package that this project was built against.
fn get_version(package: &str) -> Option<String> {
    let output = Command::new("apt-cache").arg("policy").arg(&package).output().ok().map(|x| x.stdout)
        .try("cargo-deb: apt-cache command not found. Automatic dependency resolution is only supported on Debian.");
    let string = String::from_utf8(output).unwrap();
    string.lines().nth(1).map(|installed_line| {
        let installed = installed_line.split_whitespace().nth(1).unwrap();
        if installed == "(none)" {
            failed(format!("{} is not installed", &package))
        } else {
            installed.chars().take_while(|&x| x != '-').collect()
        }
    })
}
