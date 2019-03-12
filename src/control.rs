use crate::error::*;
use crate::listener::Listener;
use crate::manifest::Config;
use crate::pathbytes::*;
use crate::tararchive::Archive;
use crate::wordsplit::WordSplit;
use md5::Digest;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Generates an uncompressed tar archive with `control`, `md5sums`, and others
pub fn generate_archive(options: &Config, time: u64, asset_hashes: HashMap<PathBuf, Digest>, listener: &mut dyn Listener) -> CDResult<Vec<u8>> {
    let mut archive = Archive::new(time);
    generate_md5sums(&mut archive, options, asset_hashes)?;
    generate_control(&mut archive, options, listener)?;
    if let Some(ref files) = options.conf_files {
        generate_conf_files(&mut archive, files)?;
    }
    generate_scripts(&mut archive, options)?;
    Ok(archive.into_inner()?)
}

/// Append all files that reside in the `maintainer_scripts` path to the archive
fn generate_scripts(archive: &mut Archive, option: &Config) -> CDResult<()> {
    if let Some(ref maintainer_scripts) = option.maintainer_scripts {
        for name in &["config", "preinst", "postinst", "prerm", "postrm"] {
            if let Ok(script) = fs::read(maintainer_scripts.join(name)) {
                archive.file(name, &script, 0o755)?;
            }
        }
    }
    Ok(())
}

/// Creates the md5sums file which contains a list of all contained files and the md5sums of each.
fn generate_md5sums(archive: &mut Archive, options: &Config, asset_hashes: HashMap<PathBuf, Digest>) -> CDResult<()> {
    let mut md5sums: Vec<u8> = Vec::new();

    // Collect md5sums from each asset in the archive.
    for asset in &options.assets.resolved {
        write!(md5sums, "{:x}", asset_hashes[&asset.target_path])?;
        md5sums.write_all(b"  ")?;

        md5sums.write_all(&asset.target_path.as_path().as_unix_path())?;
        md5sums.write_all(&[b'\n'])?;
    }

    // Write the data to the archive
    archive.file("./md5sums", &md5sums, 0o644)?;
    Ok(())
}

/// Generates the control file that obtains all the important information about the package.
fn generate_control(archive: &mut Archive, options: &Config, listener: &mut dyn Listener) -> CDResult<()> {
    // Create and return the handle to the control file with write access.
    let mut control: Vec<u8> = Vec::with_capacity(1024);

    // Write all of the lines required by the control file.
    writeln!(&mut control, "Package: {}", options.deb_name)?;
    writeln!(&mut control, "Version: {}", options.version)?;
    writeln!(&mut control, "Architecture: {}", options.architecture)?;
    if let Some(ref repo) = options.repository {
        if repo.starts_with("http") {
            writeln!(&mut control, "Vcs-Browser: {}", repo)?;
        }
        if let Some(kind) = options.repository_type() {
            writeln!(&mut control, "Vcs-{}: {}", kind, repo)?;
        }
    }
    if let Some(homepage) = options.homepage.as_ref().or(options.documentation.as_ref()) {
        writeln!(&mut control, "Homepage: {}", homepage)?;
    }
    if let Some(ref section) = options.section {
        writeln!(&mut control, "Section: {}", section)?;
    }
    writeln!(&mut control, "Priority: {}", options.priority)?;
    control.write_all(b"Standards-Version: 3.9.4\n")?;
    writeln!(&mut control, "Maintainer: {}", options.maintainer)?;

    let installed_size = options.assets.resolved
        .iter()
        .filter_map(|m| m.source.len())
        .sum::<u64>() / 1024;

    writeln!(&mut control, "Installed-Size: {}", installed_size)?;

    writeln!(&mut control, "Depends: {}", options.get_dependencies(listener)?)?;

    if let Some(ref conflicts) = options.conflicts {
        writeln!(&mut control, "Conflicts: {}", conflicts)?;
    }
    if let Some(ref breaks) = options.breaks {
        writeln!(&mut control, "Breaks: {}", breaks)?;
    }
    if let Some(ref replaces) = options.replaces {
        writeln!(&mut control, "Replaces: {}", replaces)?;
    }
    if let Some(ref provides) = options.provides {
        writeln!(&mut control, "Provides: {}", provides)?;
    }

    write!(&mut control, "Description:")?;
    for line in options.description.split_by_chars(79) {
        writeln!(&mut control, " {}", line)?;
    }

    if let Some(ref desc) = options.extended_description {
        for line in desc.split_by_chars(79) {
            writeln!(&mut control, " {}", line)?;
        }
    }
    control.push(10);

    // Add the control file to the tar archive.
    archive.file("./control", &control, 0o644)?;
    Ok(())
}

/// If configuration files are required, the conffiles file will be created.
fn generate_conf_files(archive: &mut Archive, files: &str) -> CDResult<()> {
    let mut data = Vec::new();
    data.write_all(files.as_bytes())?;
    data.push(b'\n');
    archive.file("./conffiles", &data, 0o644)?;
    Ok(())
}
