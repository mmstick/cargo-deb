use std::process::Command;
use try::{Try, failed};
use std::fmt::Write;

struct Dependency {
    name:    String,
    version: String
}

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve<S: AsRef<str>>(path: S) -> String {
    let dependencies = {
        let path = path.as_ref();
        let output = Command::new("ldd").arg(path).output().map(|x| x.stdout)
            .try("cargo-deb: failed to launch ldd command");
        let string = unsafe { String::from_utf8_unchecked(output) };
        collect_dependencies(&string)
    };

    let mut output = String::new();
    let ndependencies = dependencies.len();
    if ndependencies > 0 {
        write!(&mut output, "{} (>= {})", &dependencies[0].name, &dependencies[0].version).unwrap();
        if dependencies.len() > 1 {
            for depend in &dependencies[1..] {
                write!(&mut output, ", {} (>= {})", &depend.name, &depend.version).unwrap();
            }
        }
    }
    output
}

/// Collects a list of dependencies from the output of ldd
fn collect_dependencies(ldd: &str) -> Vec<Dependency> {
    let mut dependencies: Vec<Dependency> = Vec::new();
    let packages = ldd.lines()
        // We only want the third field on each line, which contains the filepath of the library
        .map(|line| line.split_whitespace().nth(2))
        // If the field exists and starts with '/', we have found a filepath
        .filter(|x| x.is_some() && x.unwrap().chars().next().unwrap() == '/')
        // Obtain the name of the package
        .map(|path| get_package_name(path.unwrap()));

    // Only append a package if it hasn't been appended already.
    for package in packages {
        if dependencies.iter().any(|x| &x.name != &package) {
            let version = get_version(&package);
            dependencies.push(Dependency{ name: package, version: version });
        }
    }

    dependencies
}

/// Obtains the name of the package that belongs to the file that ldd returned
fn get_package_name(path: &str) -> String {
    let output = Command::new("dpkg").arg("-S").arg(path).output().ok().map(|x| x.stdout)
        .try("cargo-deb: dpkg command not found. Automatic dependency resolution is only supported on Debian.");
    let package = output.iter().take_while(|&&x| x != b':').cloned().collect::<Vec<u8>>();
    unsafe { String::from_utf8_unchecked(package) }
}

/// Uses apt-cache policy to determine the version of the package that this project was built against.
fn get_version(package: &str) -> String {
    let output = Command::new("apt-cache").arg("policy").arg(&package).output().ok().map(|x| x.stdout)
        .try("cargo-deb: apt-cache command not found. Automatic dependency resolution is only supported on Debian.");
    let string = unsafe { String::from_utf8_unchecked(output) };
    string.lines().nth(1).map(|installed_line| {
        let installed = installed_line.split_whitespace().nth(1).unwrap();
        if installed == "(none)" {
            failed(format!("{} is not installed", &package))
        } else {
            installed.chars().take_while(|&x| x != '-').collect()
        }
    }).unwrap()
}
