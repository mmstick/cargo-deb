use crate::dh_installsystemd;
use crate::dh_lib;
use crate::error::*;
use crate::listener::Listener;
use crate::manifest::Config;
use crate::pathbytes::*;
use crate::tararchive::Archive;
use crate::util::{is_path_file, read_file_to_bytes};
use crate::wordsplit::WordSplit;
use dh_lib::ScriptFragments;
use md5::Digest;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Generates an uncompressed tar archive with `control`, `md5sums`, and others
pub fn generate_archive(options: &Config, time: u64, asset_hashes: HashMap<PathBuf, Digest>, listener: &mut dyn Listener) -> CDResult<Vec<u8>> {
    let mut archive = Archive::new(time);
    generate_md5sums(&mut archive, options, asset_hashes)?;
    generate_control(&mut archive, options, listener)?;
    if let Some(ref files) = options.conf_files {
        generate_conf_files(&mut archive, files)?;
    }
    generate_scripts(&mut archive, options, listener)?;
    if let Some(ref file) = options.triggers_file {
        generate_triggers_file(&mut archive, file)?;
    }
    Ok(archive.into_inner()?)
}

/// Append Debian maintainer script files (control, preinst, postinst, prerm,
/// postrm and templates) present in the `maintainer_scripts` path to the
/// archive, if `maintainer_scripts` is configured.
///
/// Additionally, when `systemd_units` is configured, shell script fragments
/// "for enabling, disabling, starting, stopping and restarting systemd unit
/// files" (quoting man 1 dh_installsystemd) will replace the `#DEBHELPER#`
/// token in the provided maintainer scripts.
///
/// If a shell fragment cannot be inserted because the target script is missing
/// then the entire script will be generated and appended to the archive.
///
/// # Requirements
///
/// When `systemd_units` is configured, user supplied `maintainer_scripts` must
/// contain a `#DEBHELPER#` token at the point where shell script fragments
/// should be inserted.
fn generate_scripts(archive: &mut Archive, option: &Config, listener: &mut dyn Listener) -> CDResult<()> {
    if let Some(ref maintainer_scripts_dir) = option.maintainer_scripts {
        let mut scripts;

        if let Some(systemd_units_config) = &option.systemd_units {
            // Select and populate autoscript templates relevant to the unit
            // file(s) in this package and the configuration settings chosen.
            scripts = dh_installsystemd::generate(
                &option.name,
                &option.assets.resolved,
                &dh_installsystemd::Options::from(systemd_units_config),
                listener)?;

            // Get Option<&str> from Option<String>
            let unit_name = systemd_units_config.unit_name
                .as_deref();

            // Replace the #DEBHELPER# token in the users maintainer scripts
            // and/or generate maintainer scripts from scratch as needed.
            dh_lib::apply(
                &maintainer_scripts_dir,
                &mut scripts,
                &option.name,
                unit_name,
                listener)?;
        } else {
            scripts = ScriptFragments::with_capacity(0);
        }

        // Add maintainer scripts to the archive, either those supplied by the
        // user or if available prefer modified versions generated above.
        for name in &["config", "preinst", "postinst", "prerm", "postrm", "templates"] {
            let mut script = scripts.remove(&name.to_string());

            if script.is_none() {
                let script_path = maintainer_scripts_dir.join(name);
                if is_path_file(&script_path) {
                    script = Some(read_file_to_bytes(&script_path)?);
                }
            }

            if let Some(contents) = script {
                archive.file(name, &contents, 0o755)?;
            }
        }
    }

    Ok(())
}

/// Creates the md5sums file which contains a list of all contained files and the md5sums of each.
fn generate_md5sums(archive: &mut Archive, options: &Config, asset_hashes: HashMap<PathBuf, Digest>) -> CDResult<()> {
    let mut md5sums: Vec<u8> = Vec::new();

    // Collect md5sums from each asset in the archive (excludes symlinks).
    for asset in &options.assets.resolved {
        if let Some(value) = asset_hashes.get(&asset.target_path) {
            write!(md5sums, "{:x}", value)?;
            md5sums.write_all(b"  ")?;

            md5sums.write_all(&asset.target_path.as_path().as_unix_path())?;
            md5sums.write_all(&[b'\n'])?;
        }
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
    writeln!(&mut control, "Version: {}", options.deb_version)?;
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

    let deps = options.get_dependencies(listener)?;
    if !deps.is_empty() {
        writeln!(&mut control, "Depends: {}", deps)?;
    }

    if let Some(ref build_depends) = options.build_depends {
        writeln!(&mut control, "Build-Depends: {}", build_depends)?;
    }

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

fn generate_triggers_file(archive: &mut Archive, path: &Path) -> CDResult<()> {
    if let Ok(content) = fs::read(path) {
        archive.file("./triggers", &content, 0o644)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Asset, AssetSource, SystemdUnitsConfig};
    use crate::util::tests::{add_test_fs_paths, set_test_fs_path_content};
    use std::io::prelude::Read;

    fn decode_name<R>(entry: &tar::Entry<R>) -> String where R: Read {
        std::str::from_utf8(&entry.path_bytes()).unwrap().to_string()
    }

    fn decode_names<R>(ar: &mut tar::Archive<R>) -> Vec<String> where R: Read {
        ar.entries().unwrap().map(|e| decode_name(&e.unwrap())).collect()
    }

    fn extract_contents<R>(ar: &mut tar::Archive<R>) -> HashMap<String, String> where R: Read {
        let mut out = HashMap::new();
        for entry in ar.entries().unwrap() {
            let mut unwrapped = entry.unwrap();
            let name = decode_name(&unwrapped);
            let mut buf = Vec::new();
            unwrapped.read_to_end(&mut buf).unwrap();
            let content = String::from_utf8(buf).unwrap();
            out.insert(name, content);
        }
        out
    }

    fn prepare() -> (Config, crate::listener::MockListener, Archive) {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().return_const(());

        let config = Config::from_manifest(
            Path::new("Cargo.toml"),
            None,
            None,
            None,
            None,
            None,
            &mut mock_listener,
        ).unwrap();

        let ar = Archive::new(0);

        (config, mock_listener, ar)
    }

    #[test]
    fn generate_scripts_does_nothing_if_maintainer_scripts_is_not_set() {
        let (config, mut mock_listener, mut in_ar) = prepare();

        // supply a maintainer script as if it were available on disk
        add_test_fs_paths(&vec!["debian/postinst"]);

        // generate scripts and store them in the given archive
        generate_scripts(&mut in_ar, &config, &mut mock_listener).unwrap();

        // finish the archive and unwrap it as a byte vector
        let archive_bytes = in_ar.into_inner().unwrap();

        // parse the archive bytes
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        // compare the file names in the archive to what we expect
        let archived_file_names = decode_names(&mut out_ar);
        assert!(archived_file_names.is_empty());
    }

    #[test]
    fn generate_scripts_archives_user_supplied_maintainer_scripts() {
        let maintainer_script_names = &vec!["config", "preinst", "postinst", "prerm", "postrm", "templates"];

        let (mut config, mut mock_listener, mut in_ar) = prepare();

        // supply a maintainer script as if it were available on disk
        // provide file content that we can easily verify
        let mut maintainer_script_contents = Vec::new();
        for script in maintainer_script_names.iter() {
            let content = format!("some contents: {}", script);
            set_test_fs_path_content(script, content.clone());
            maintainer_script_contents.push(content);
        }

        // look in the current (virtual) dir for the maintainer script we just
        // "added"
        config.maintainer_scripts.get_or_insert(PathBuf::new());

        // generate scripts and store them in the given archive
        generate_scripts(&mut in_ar, &config, &mut mock_listener).unwrap();

        // finish the archive and unwrap it as a byte vector
        let archive_bytes = in_ar.into_inner().unwrap();

        // parse the archive bytes
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        // compare the file names in the archive to what we expect
        // let archived_file_names = decode_names(&mut out_ar);
        // assert_eq!(maintainer_script_names.to_owned(), archived_file_names);

        // compare the file contents in the archive to what we expect
        let archived_content = extract_contents(&mut out_ar);

        assert_eq!(maintainer_script_names.len(), archived_content.len());

        // verify that the content we supplied was faithfully archived
        for script in maintainer_script_names.iter() {
            let expected_content = &format!("some contents: {}", script);
            let actual_content = archived_content.get(&script.to_string()).unwrap();
            assert_eq!(expected_content, actual_content);
        }
    }

    #[test]
    fn generate_scripts_generates_maintainer_scripts_for_unit() {
        let (mut config, mut mock_listener, mut in_ar) = prepare();

        // supply a systemd unit file as if it were available on disk
        add_test_fs_paths(&vec!["some.service"]);

        // make the unit file available for systemd unit processing
        config.assets.resolved.push(Asset::new(
            AssetSource::Path(PathBuf::from("some.service")),
            PathBuf::from("lib/systemd/system/some.service"),
            0o000,
            false,
        ));

        // look in the current dir for maintainer scripts (none, but the systemd
        // unit processing will be skipped if we don't set this)
        config.maintainer_scripts.get_or_insert(PathBuf::new());

        // enable systemd unit processing
        config.systemd_units.get_or_insert(SystemdUnitsConfig::default());

        // generate scripts and store them in the given archive
        generate_scripts(&mut in_ar, &config, &mut mock_listener).unwrap();

        // finish the archive and unwrap it as a byte vector
        let archive_bytes = in_ar.into_inner().unwrap();

        // parse the archive bytes
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        // compare the file names in the archive to what we expect
        let mut archived_file_names = decode_names(&mut out_ar);
        archived_file_names.sort();

        // don't check the file content, generation of correct files and content
        // is tested at a lower level, we're only testing the higher level.
        let expected_maintainer_scripts = vec!["postinst", "postrm", "prerm"]
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>();
        assert_eq!(expected_maintainer_scripts, archived_file_names);
    }
}
