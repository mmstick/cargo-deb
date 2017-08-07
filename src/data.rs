use std::io::Write;
use std::path::PathBuf;
use md5::Digest;
use md5;
use file;
use manifest::Config;
use std::collections::HashMap;
use error::*;
use archive::Archive;

/// Generates the uncompressed control.tar archive
pub fn generate_archive(options: &Config, time: u64) -> CDResult<(Vec<u8>, HashMap<PathBuf, Digest>)> {
    let mut archive = Archive::new(time);
    generate_copyright_asset(options)?;
    let copy_hashes = archive_files(&mut archive, options)?;
    Ok((archive.into_inner()?, copy_hashes))
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
            .map_err(|e| CargoDebError::IoFile(e, path.to_owned()))?;
        // Skip the first `A` number of lines and then iterate each line after that.
        for line in license_string.lines().skip(options.license_file_skip_lines) {
            // If the line is empty, write a dot, else write the line.
            if line.is_empty() {
                copyright.write(b".\n")?;
            } else {
                copyright.write(line.trim().as_bytes())?;
                copyright.write(b"\n")?;
            }
        }
    }

    // Write a copy to the disk for the sake of obtaining a md5sum for the control archive.
    file::put(options.path_in_deb("copyright"), &copyright)?;
    Ok(())
}

/// Copies all the files to be packaged into the tar archive.
/// Returns MD5 hashes of files copied
fn archive_files(archive: &mut Archive, options: &Config) -> CDResult<HashMap<PathBuf, Digest>> {
    let mut hashes = HashMap::new();
    for asset in &options.assets {
        let out_data = file::get(&asset.source_file)
            .map_err(|e| CargoDebError::IoFile(e, asset.source_file.clone()))?;

        hashes.insert(asset.source_file.clone(), md5::compute(&out_data));
        archive.file(&asset.target_path, &out_data, asset.chmod)?;
    }
    Ok(hashes)
}
