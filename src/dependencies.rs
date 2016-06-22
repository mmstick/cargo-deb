use std::process::{exit, Command};

struct Dependency {
    name:    String,
    version: String
}

/// Resolves the dependencies based on the output of ldd on the binary.
pub fn resolve<S: AsRef<str>>(path: S) -> String {
    let path = path.as_ref();
    let output = Command::new("ldd").arg(path).output().unwrap().stdout;
    let string = unsafe { String::from_utf8_unchecked(output) };
    let dependencies = collect_dependencies(&string);
    let mut output = String::new();
    for depend in &dependencies {
        output.push_str(&depend.name);
        output.push_str(" (>= ");
        output.push_str(&depend.version);
        output.push_str("), ");
    }
    let capacity = output.chars().count();
    output.truncate(capacity-2);
    output
}

/// Collects a list of dependencies from the output of ldd
fn collect_dependencies(ldd: &str) -> Vec<Dependency> {
    let mut dependencies: Vec<Dependency> = Vec::new();
    for line in ldd.lines() {
        let mut words = line.split_whitespace();
        let word = words.nth(2);
        if word.is_none() { continue }
        let path = word.unwrap();
        if path.chars().next().unwrap() == '/' {
            let package = get_package_name(path);
            if dependencies.iter().any(|x| &x.name == &package) { continue }
            match get_version(&package) {
                Some(version) => dependencies.push(Dependency{name: package, version: version}),
                None => continue
            }
        }
    }
    dependencies
}

/// Obtains the name of the package that belongs to the file that ldd returned
fn get_package_name(path: &str) -> String {
    let package = Command::new("dpkg").arg("-S").arg(path).output().unwrap().stdout.iter()
        .take_while(|&&x| x != b':').cloned().collect::<Vec<u8>>();
    unsafe { String::from_utf8_unchecked(package) }
}

/// Uses apt-cache policy to determine the version of the package that this project was built against.
fn get_version(package: &str) -> Option<String> {
    let output = Command::new("apt-cache").arg("policy").arg(&package)
        .output().unwrap().stdout;
    let string = unsafe { String::from_utf8_unchecked(output) };
    string.lines().nth(1).map(|installed_line| {
        let installed = installed_line.split_whitespace().nth(1).unwrap();
        if installed == "(none)" {
            println!("{} is not installed", &package);
            exit(1);
        } else {
            installed.chars().take_while(|&x| x != '-').collect()
        }
    })
}
