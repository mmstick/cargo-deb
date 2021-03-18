use crate::error::*;
use crate::listener::Listener;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve(path: &Path, architecture: &str, listener: &dyn Listener) -> CDResult<Vec<String>> {
    let dependencies = {
        let output = Command::new("ldd")
            .arg(path)
            .output()
            .map_err(|e| CargoDebError::CommandFailed(e, "ldd"))?;
        if !output.status.success() {
            return Err(CargoDebError::CommandError("ldd", path.display().to_string(), output.stderr));
        }
        String::from_utf8(output.stdout).expect("utf8")
    };

    // Create an iterator of unique dependencies
    let dependencies: HashSet<_> = dependencies.lines()
        // The syntax is "name => path (addr)"
        .filter_map(|line| {
            let mut parts = line.splitn(2, "=>");
            let name = parts.next()?.trim();

            if name == "libgcc_s.so.1" {
                // it's guaranteed by LSB to always be present
                return None;
            }
            parts.next()?.split_whitespace().next()
        })
        // If the field exists and starts with '/', we have found a filepath.
        .filter(|x| x.starts_with('/'))
        // Obtain the names of the packages.
        .filter_map(|path_str| {
            get_package_name_with_fallback(path_str)
                .map_err(|err| {
                    listener.warning(format!("{} (skip this auto dep for {})", err, path.display()));
                    err
                })
                .ok()
        })
        // only collect unique packages.
        .collect();

    Ok(dependencies.into_par_iter().map(|package| {
        // There can be multiple arch-specific versions of a package
        let arch_version = format!("{}:{}", package, architecture);
        match get_version(&arch_version) {
            Ok(version) => {
                format!("{} (>= {})", package, version)
            },
            Err(e) => {
                listener.warning(format!("Can't get version of {}: {}", arch_version, e));
                package
            },
        }
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
    if output.status.success() {
        if let Some(name) = parse_dpkg_search(&output.stdout) {
            return Ok(name);
        }
    }
    Err(CargoDebError::PackageNotFound(path.to_owned(), output.stderr))
}

fn parse_dpkg_search(output: &[u8]) -> Option<String> {
    let output = String::from_utf8_lossy(output);
    for line in output.lines() {
        if line.starts_with("diversion ") || !line.contains(':') {
            continue;
        }
        let mut parts = line.splitn(2, ':');
        if let Some(name) = parts.next() {
            return Some(name.to_owned());
        }
    }
    None
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
    let version = ::std::str::from_utf8(&output.stdout).expect("utf8");
    Ok(version.splitn(2, '-').next().unwrap().to_owned())
}

#[test]
#[cfg(target_os = "linux")]
fn resolve_test() {
    let exe = std::env::current_exe().unwrap();
    let arch = crate::manifest::get_arch(crate::DEFAULT_TARGET);
    let deps = resolve(&exe, arch, &crate::listener::NoOpListener).unwrap();
    assert!(deps.iter().any(|d| d.starts_with("libc")));
    assert!(!deps.iter().any(|d| d.starts_with("libgcc")));
}

#[test]
fn parse_search() {
    assert_eq!("libgl1-mesa-glx", parse_dpkg_search(b"diversion by glx-diversions from: /usr/lib/x86_64-linux-gnu/libGL.so.1
diversion by glx-diversions to: /usr/lib/mesa-diverted/x86_64-linux-gnu/libGL.so.1
libgl1-mesa-glx:amd64: /usr/lib/x86_64-linux-gnu/libGL.so.1
").unwrap());
}
