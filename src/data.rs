use crate::error::*;
use crate::listener::Listener;
use crate::manifest::{Asset, Config};
use crate::tararchive::Archive;
use md5::Digest;
use std::collections::HashMap;
use std::fmt;
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

fn append_copyright_metadata(copyright: &mut Vec<u8>, options: &Config) -> Result<(), CargoDebError> {
    writeln!(copyright, "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/")?;
    writeln!(copyright, "Upstream-Name: {}", options.name)?;
    if let Some(source) = options.repository.as_ref().or(options.homepage.as_ref()) {
        writeln!(copyright, "Source: {}", source)?;
    }
    writeln!(copyright, "Copyright: {}", options.copyright)?;
    if let Some(ref license) = options.license {
        writeln!(copyright, "License: {}", license)?;
    }
    Ok(())
}

/// Generates the copyright file from the license file and adds that to the tar archive.
pub(crate) fn generate_copyright_asset(options: &Config) -> CDResult<Vec<u8>> {
    let mut copyright: Vec<u8> = Vec::new();
    if let Some(ref path) = options.license_file {
        let license_string = fs::read_to_string(options.path_in_workspace(path))
            .map_err(|e| CargoDebError::IoFile("unable to read license file", e, path.to_owned()))?;
        if !has_copyright_metadata(&license_string) {
            append_copyright_metadata(&mut copyright, options)?;
        }

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
    } else {
        append_copyright_metadata(&mut copyright, options)?;
    }

    // Write a copy to the disk for the sake of obtaining a md5sum for the control archive.
    Ok(copyright)
}

fn has_copyright_metadata(file: &str) -> bool {
    file.lines().take(10)
        .any(|l| l.starts_with("License: ") || l.starts_with("Source: ") || l.starts_with("Upstream-Name: ") || l.starts_with("Format: "))
}

/// Compress man page assets per Debian Policy.
///
/// # References
///
/// https://www.debian.org/doc/debian-policy/ch-docs.html#manual-pages
/// https://lintian.debian.org/tags/manpage-not-compressed.html
pub fn compress_man_pages(options: &mut Config, listener: &dyn Listener) -> CDResult<()> {
    let mut indices_to_remove = Vec::new();
    let mut new_assets = Vec::new();

    for (idx, asset) in options.assets.resolved.iter().enumerate() {
        let target_path_str = asset.target_path.to_string_lossy();
        if target_path_str.starts_with("usr/share/man/") &&
           !target_path_str.ends_with(".gz")
        {
            listener.info(format!("Compressing '{}'", asset.source.path().unwrap_or(Path::new("-")).display()));

            let content = asset.source.data()?;
            let mut compressed = Vec::with_capacity(content.len());
            zopfli::compress(&Options::default(), &Format::Gzip, &content, &mut compressed)?;
            compressed.shrink_to_fit();

            new_assets.push(Asset::new(
                crate::manifest::AssetSource::Data(compressed),
                Path::new(&format!("{}.gz", target_path_str)).into(),
                asset.chmod,
                false,
            ));

            indices_to_remove.push(idx);
        }
    }

    for idx in indices_to_remove.iter().rev() {
        options.assets.resolved.remove(*idx);
    }

    options.assets.resolved.append(&mut new_assets);

    Ok(())
}

/// Copies all the files to be packaged into the tar archive.
/// Returns MD5 hashes of files copied
fn archive_files(archive: &mut Archive, options: &Config, listener: &mut dyn Listener) -> CDResult<HashMap<PathBuf, Digest>> {
    let mut hashes = HashMap::new();
    for asset in &options.assets.resolved {
        let out_data = asset.source.data()?;

        let mut log_line = format!(
            "{} -> {}",
            asset.source.path().unwrap_or_else(|| Path::new("-")).display(),
            asset.target_path.display()
        );
        if let Some(len) = asset.source.len() {
            let (size, unit) = human_size(len);
            let _ = fmt::Write::write_fmt(&mut log_line, format_args!(" ({}{})", size, unit));
        }
        listener.info(log_line);

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

fn human_size(len: u64) -> (u64, &'static str) {
    if len < 1000 {
        return (len, "B");
    }
    if len < 1000_000 {
        return ((len + 999) / 1000, "KB");
    }
    return ((len + 999_999) / 1000_000, "MB");
}
