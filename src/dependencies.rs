use std::process::Command;
use std::fmt::Write;
use itertools::Itertools;
use std::collections::HashSet;
use error::*;

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve(path: &str) -> CDResult<String> {
    let dependencies = {
        let output = Command::new("ldd").arg(path).output().map(|x| x.stdout)?;
        String::from_utf8(output).unwrap()
    };

    // Create an iterator of unique dependencies
    let dependencies: Result<HashSet<_>,_> = dependencies.lines()
        // We only want the third field on each line, which contains the filepath of the library.
        .map(|line| line.split_whitespace().nth(2))
        // If the field exists and starts with '/', we have found a filepath.
        .filter(|x| x.is_some() && x.unwrap().chars().next().unwrap() == '/')
        // Obtain the names of the packages.
        .map(|path| get_package_name(path.unwrap()))
        // only collect unique packages.
        .collect();

    // Create a formatted string with the output from ldd.
    let mut output = String::with_capacity(256);
    for package in dependencies? {
        let version = get_version(&package)?;
        if !output.is_empty() {
            output += ", ";
        }
        write!(&mut output, "{} (>= {})", package, &version).unwrap();
    }
    Ok(output)
}

/// Obtains the name of the package that belongs to the file that ldd returned.
fn get_package_name(path: &str) -> CDResult<String> {
    let output = Command::new("dpkg").arg("-S").arg(path).output()?;
    if !output.status.success() {
        return Err(CargoDebError::PackageNotFound(path.to_owned(), output.stderr));
    }
    let package = output.stdout.iter().take_while(|&&x| x != b':').cloned().collect::<Vec<u8>>();
    Ok(String::from_utf8(package).unwrap())
}

/// Uses apt-cache policy to determine the version of the package that this project was built against.
fn get_version(package: &str) -> CDResult<String> {
    let output = Command::new("apt-cache").arg("policy").arg(&package).output()?;
    if !output.status.success() {
        return Err(CargoDebError::AptPolicyFailed(package.to_owned(), output.stderr));
    }
    let string = String::from_utf8(output.stdout).unwrap();
    if let Some(installed_line) = string.lines().nth(1) {
        let installed = installed_line.split(":").skip(1).join(":").trim().to_owned();
        if installed.starts_with('(') && installed.ends_with(')') { // "(none)" or localised "(none)" in other languages
            Err(CargoDebError::NotInstalled(package.to_owned()))
        } else {
            Ok(installed.chars().take_while(|&x| x != '-').collect())
        }
    } else {
        Err(CargoDebError::GetVersionError(package.to_owned()))
    }
}
