use crate::error::*;
use crate::listener::Listener;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve(path: &Path, architecture: &str, listener: &mut dyn Listener) -> CDResult<Vec<String>> {
    let dependencies = {
        let output = Command::new("ldd")
            .arg(path)
            .output()
            .map_err(|e| CargoDebError::CommandFailed(e, "ldd"))?;
        if !output.status.success() {
            return Err(CargoDebError::CommandError("ldd", path.display().to_string(), output.stderr));
        }
        String::from_utf8(output.stdout).unwrap()
    };

    // Create an iterator of unique dependencies
    let dependencies: HashSet<_> = dependencies.lines()
        // We only want the third field on each line, which contains the filepath of the library.
        .map(|line| line.split_whitespace().nth(2))
        // If the field exists and starts with '/', we have found a filepath.
        .filter(|x| x.is_some() && x.unwrap().starts_with('/'))
        // Obtain the names of the packages.
        .filter_map(|path_str_opt| {
            get_package_name_with_fallback(path_str_opt.unwrap())
                .map_err(|err| {
                    listener.warning(format!(
                        "{} (skip this auto dep for {})",
                        err,
                        path.display()
                    ));
                    err
                })
                .ok()
        })
        // only collect unique packages.
        .collect();

    Ok(dependencies.iter().map(|package| {
        // There can be multiple arch-specific versions of a package
        let version = get_version(&format!("{}:{}", package, architecture)).unwrap();   /* If we got here, package exists. */
        format!("{} (>= {})", package, version)
    }).collect())
}

/// Debian's libssl links with a lib that isn't "installed", #26
/// but exists in /usr/lib instead of /lib
fn get_package_name_with_fallback(path: &str) -> CDResult<String> {
    match get_package_name(path) {
        Ok(res) => Ok(res),
        Err(e @ CargoDebError::PackageNotFound(..)) => {
            let usr_path = format!("/usr{}", path);
            match get_package_name(&usr_path) {
                Ok(res) => Ok(res),
                _ => Err(e),
            }
        },
        Err(e) => Err(e),
    }
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
    let output = Command::new("dpkg-query")
        .arg("--showformat=${Version}")
        .arg("--show")
        .arg(package)
        .output()
        .map_err(|e| CargoDebError::CommandFailed(e, "dpkg-query (get package version)"))?;
    if !output.status.success() {
        return Err(CargoDebError::CommandError("dpkg-query (get package version)", package.to_owned(), output.stderr));
    }
    let version = ::std::str::from_utf8(&output.stdout).unwrap();
    Ok(version.splitn(2, '-').next().unwrap().to_owned())
}
