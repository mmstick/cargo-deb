use crate::error::*;
use crate::listener::Listener;
use crate::manifest::Config;
use crate::tararchive::Archive;
use md5::Digest;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zopfli::{self, Format, Options};

/// Generates an uncompressed tar archive and hashes of its files
pub fn generate_archive(options: &Config, time: u64, listener: &mut dyn Listener) -> CDResult<(Vec<u8>, HashMap<PathBuf, Digest>)> {
    let mut archive = Archive::new(time);
    let copy_hashes = archive_files(&mut archive, options, listener)?;
    Ok((archive.into_inner()?, copy_hashes))
}

/// Generates compressed changelog file
pub(crate) fn generate_changelog_asset(options: &Config) -> CDResult<Option<Vec<u8>>> {
    if let Some(ref path) = options.changelog {
        let changelog = fs::read(options.path_in_workspace(path))
            .and_then(|content| {
                // The input is plaintext, but the debian package should contain gzipped one.
                let mut compressed = Vec::with_capacity(content.len());
                zopfli::compress(&Options::default(), &Format::Gzip, &content, &mut compressed)?;
                compressed.shrink_to_fit();
                Ok(compressed)
            })
            .map_err(|e| CargoDebError::IoFile("unable to read changelog file", e, path.into()))?;
        Ok(Some(changelog))
    } else {
        Ok(None)
    }
}

/// Generates the copyright file from the license file and adds that to the tar archive.
pub(crate) fn generate_copyright_asset(options: &Config) -> CDResult<Vec<u8>> {
    let mut copyright: Vec<u8> = Vec::new();
    writeln!(&mut copyright, "Upstream Name: {}", options.name)?;
    if let Some(source) = options.repository.as_ref().or(options.homepage.as_ref()) {
        writeln!(&mut copyright, "Source: {}", source)?;
    }
    writeln!(&mut copyright, "Copyright: {}", options.copyright)?;
    if let Some(ref license) = options.license {
        writeln!(&mut copyright, "License: {}", license)?;
    }
    if let Some(ref path) = options.license_file {
        let license_string = fs::read_to_string(options.path_in_workspace(path))
            .map_err(|e| CargoDebError::IoFile("unable to read license file", e, path.to_owned()))?;
        // Skip the first `A` number of lines and then iterate each line after that.
        for line in license_string.lines().skip(options.license_file_skip_lines) {
            // If the line is a space, add a dot, else write the line.
            if line == " " {
                copyright.write_all(b" .\n")?;
            } else {
                copyright.write_all(line.as_bytes())?;
                copyright.write_all(b"\n")?;
            }
        }
    }

    // Write a copy to the disk for the sake of obtaining a md5sum for the control archive.
    Ok(copyright)
}

/// Copies all the files to be packaged into the tar archive.
/// Returns MD5 hashes of files copied
fn archive_files(archive: &mut Archive, options: &Config, listener: &mut dyn Listener) -> CDResult<HashMap<PathBuf, Digest>> {
    let mut hashes = HashMap::new();
    for asset in &options.assets.resolved {
        let out_data = asset.source.data()?;

        listener.info(format!("{} -> {}", asset.source.path().unwrap_or_else(|| Path::new("-")).display(), asset.target_path.display()));

        let mut archived = false;
        if options.preserve_symlinks {
            if let Some(source_path) = asset.source.path() {
                let md = fs::symlink_metadata(source_path)?;
                if md.file_type().is_symlink() {
                    archived = true;
                    let link_name = fs::read_link(source_path)?;
                    archive.symlink(&asset.target_path, &link_name)?;
                }
            }
        }

        if !archived {
            hashes.insert(asset.target_path.clone(), md5::compute(&out_data));
            archive.file(&asset.target_path, &out_data, asset.chmod)?;
        }
    }
    Ok(hashes)
}
