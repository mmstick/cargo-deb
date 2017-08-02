use std::process::Command;
use std::path::Path;
use std::collections::HashSet;
use error::*;

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve(path: &Path) -> CDResult<Vec<String>> {
    let dependencies = {
        let output = Command::new("ldd").arg(path)
            .output().map_err(|e| CargoDebError::CommandFailed(e, "ldd"))?;
        if !output.status.success() {
            return Err(CargoDebError::CommandError("ldd", path.display().to_string(), output.stderr));
        }
        String::from_utf8(output.stdout).unwrap()
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

    Ok(dependencies?.iter().map(|package| {
        let version = get_version(&package).unwrap();   /* If we got here, package exists. */
        format!("{} (>= {})", package, version)
    }).collect())
}

/// Obtains the name of the package that belongs to the file that ldd returned.
fn get_package_name(path: &str) -> CDResult<String> {
    let output = Command::new("dpkg").arg("-S").arg(path)
        .output().map_err(|e| CargoDebError::CommandFailed(e, "dpkg -S"))?;
    if !output.status.success() {
        return Err(CargoDebError::PackageNotFound(path.to_owned(), output.stderr));
    }
    let package = output.stdout.iter().take_while(|&&x| x != b':').cloned().collect::<Vec<u8>>();
    Ok(String::from_utf8(package).unwrap())
}

/// Uses apt-cache policy to determine the version of the package that this project was built against.
fn get_version(package: &str) -> CDResult<String> {
    let output = Command::new("dpkg").arg("-s").arg(&package)
        .output().map_err(|e| CargoDebError::CommandFailed(e, "dpkg -s"))?;
    if !output.status.success() {
        return Err(CargoDebError::CommandError("dpkg -s", package.to_owned(), output.stderr));
    }
    parse_version(package, ::std::str::from_utf8(&output.stdout).unwrap())
}

fn parse_version(package: &str, apt_cache_out: &str) -> CDResult<String> {
    let version_lines = apt_cache_out.lines().filter(|l| l.starts_with("Version:"));
    let mut version = version_lines.filter_map(|line| line.splitn(2,':').skip(1).next()).map(|v|v.trim());

    if let Some(version) = version.next() {
        Ok(version.splitn(2, '-').next().unwrap().to_owned())
    } else {
        Err(CargoDebError::GetVersionError(package.to_owned()))
    }
}

#[test]
fn parse_version_test() {
    assert_eq!(parse_version("foopackage", r"Package: libc6
Status: install ok installed
Priority: required
Architecture: amd64
Version: 2.23-0ubuntu9
Multi-Arch: same
Source: glibc"
).unwrap(), "2.23");
}
