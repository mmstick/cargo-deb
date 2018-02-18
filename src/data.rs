use std::io::Write;
use std::path::PathBuf;
use md5::Digest;
use md5;
use file;
use compress;
use manifest::Config;
use std::collections::HashMap;
use error::*;
use archive::Archive;
use listener::Listener;

/// Generates an uncompressed tar archive and hashes of its files
pub fn generate_archive(options: &Config, time: u64, listener: &mut Listener) -> CDResult<(Vec<u8>, HashMap<PathBuf, Digest>)> {
    let mut archive = Archive::new(time);
    generate_copyright_asset(options)?;
    generate_changelog_asset(options)?;
    let copy_hashes = archive_files(&mut archive, options, listener)?;
    Ok((archive.into_inner()?, copy_hashes))
}

/// Generates compressed changelog file
fn generate_changelog_asset(options: &Config) -> CDResult<()> {
    if let Some(ref path) = options.changelog {
        let changelog = file::get(options.workspace_root.join(path))
            .map_err(|e| CargoDebError::IoFile("unable to read changelog file", e, path.into()))?;
        compress::gz(&changelog, &options.path_in_deb("changelog"))?;
    }
    Ok(())
}

/// Generates the copyright file from the license file and adds that to the tar archive.
fn generate_copyright_asset(options: &Config) -> CDResult<()> {
    let mut copyright: Vec<u8> = Vec::new();
    write!(&mut copyright, "Upstream Name: {}\n", options.name)?;
    if let Some(source) = options.repository.as_ref().or(options.homepage.as_ref()) {
        write!(&mut copyright, "Source: {}\n", source)?;
    }
    write!(&mut copyright, "Copyright: {}\n", options.copyright)?;
    if let Some(ref license) = options.license {
        write!(&mut copyright, "License: {}\n", license)?;
    }
    if let Some(ref path) = options.license_file {
        let license_string = file::get_text(path)
            .map_err(|e| CargoDebError::IoFile("unable to read license file", e, path.to_owned()))?;
        // Skip the first `A` number of lines and then iterate each line after that.
        for line in license_string.lines().skip(options.license_file_skip_lines) {
            // If the line is empty, write a dot, else write the line.
            if line.is_empty() {
                copyright.write_all(b".\n")?;
            } else {
                copyright.write_all(line.trim().as_bytes())?;
                copyright.write_all(b"\n")?;
            }
        }
    }

    // Write a copy to the disk for the sake of obtaining a md5sum for the control archive.
    file::put(options.path_in_deb("copyright"), &copyright)?;
    Ok(())
}

/// Copies all the files to be packaged into the tar archive.
/// Returns MD5 hashes of files copied
fn archive_files(archive: &mut Archive, options: &Config, listener: &mut Listener) -> CDResult<HashMap<PathBuf, Digest>> {
    let mut hashes = HashMap::new();
    for asset in &options.assets {
        let out_data = file::get(&asset.source_file)
            .map_err(|e| CargoDebError::IoFile("unable to read asset to add to archive", e, asset.source_file.clone()))?;

        listener.info(format!("{} -> {}", asset.source_file.display(), asset.target_path.display()));

        hashes.insert(asset.source_file.clone(), md5::compute(&out_data));
        archive.file(&asset.target_path, &out_data, asset.chmod)?;
    }
    Ok(hashes)
}
