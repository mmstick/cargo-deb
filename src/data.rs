use std::fs;
use std::io::{self, Write};
use std::path::{PathBuf, Component};
use std::os::unix::fs::OpenOptionsExt;
use tar::Header as TarHeader;
use tar::Builder as TarBuilder;
use tar::EntryType;
use md5::Digest;
use md5;
use file;
use config::Config;
use std::collections::{HashMap, HashSet};
use error::*;
const CHMOD_FILE:       u32 = 420;
const CHMOD_BIN_OR_DIR: u32 = 493;

/// Generates the uncompressed control.tar archive
pub fn generate_archive(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) -> CDResult<HashMap<PathBuf, Digest>> {
    let copy_hashes = copy_files(archive, options, time)?;
    generate_copyright(archive, options, time)?;
    Ok(copy_hashes)
}

/// Generates the copyright file from the license file and adds that to the tar archive.
fn generate_copyright(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) -> CDResult<()> {
    let mut copyright: Vec<u8> = Vec::new();
    write!(&mut copyright, "Upstream Name: {}\n", options.name)?;
    if let Some(source) = options.repository.as_ref().or(options.homepage.as_ref()) {
        write!(&mut copyright, "Source: {}\n", source)?;
    }
    write!(&mut copyright, "Copyright: {}\n", options.copyright)?;
    write!(&mut copyright, "License: {}\n", options.license)?;
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
    {
        let mut copyright_file = fs::OpenOptions::new().create(true).write(true).truncate(true).mode(CHMOD_FILE)
            .open(options.path_in_deb("copyright"))?;
        copyright_file.write_all(copyright.as_slice())?;
    }
    let target = format!("./usr/share/doc/{}/", options.name);

    for dir in &[".", "./usr/", "./usr/share/", "./usr/share/doc/", target.as_str()] {
        if ::TAR_REJECTS_CUR_DIR && dir == &"." {
            continue;
        }

        let mut header = TarHeader::new_gnu();
        header.set_mtime(time);
        header.set_size(0);
        header.set_mode(CHMOD_BIN_OR_DIR);
        header.set_path(&dir)?;
        header.set_entry_type(EntryType::Directory);
        header.set_cksum();
        archive.append(&header, &mut io::empty())?;
    }

    // Now add a copy to the archive
    let mut header = TarHeader::new_gnu();
    header.set_mtime(time);
    header.set_path(&(target + "copyright"))?;
    header.set_size(copyright.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, copyright.as_slice())?;
    Ok(())
}

/// Copies all the files to be packaged into the tar archive.
/// Returns MD5 hashes of files copied
fn copy_files(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) -> CDResult<HashMap<PathBuf, Digest>> {
    let mut hashes = HashMap::new();
    let mut added_directories: HashSet<PathBuf> = HashSet::new();
    for asset in &options.assets {

        // Append each of the directories found in the file's pathname to the archive before adding the file
        // For each directory pathname found, attempt to add it to the list of directories
        let asset_relative_dir = PathBuf::from(".").join(asset.target_path.parent().ok_or("invalid asset")?);
        let mut directory = PathBuf::new();
        for comp in asset_relative_dir.components() {
            match comp {
                Component::CurDir if !::TAR_REJECTS_CUR_DIR => directory.push("."),
                Component::Normal(c) => directory.push(c),
                _ => continue,
            }
            if !added_directories.contains(&directory) {
                added_directories.insert(directory.clone());

                let mut header = TarHeader::new_gnu();
                header.set_mtime(time);
                header.set_size(0);
                header.set_mode(CHMOD_BIN_OR_DIR);
                header.set_path(&directory)?;
                header.set_entry_type(EntryType::Directory);
                header.set_cksum();
                archive.append(&header, &mut io::empty())?;
            }
        }

        // Add the file to the archive
        let out_data = file::get(&asset.source_file)
            .map_err(|e| CargoDebError::IoFile(e, asset.source_file.display().to_string()))?;

        hashes.insert(asset.source_file.clone(), md5::compute(&out_data));

        let mut header = TarHeader::new_gnu();
        header.set_mtime(time);
        header.set_path(&asset.target_path)?;
        header.set_mode(asset.chmod);
        header.set_size(out_data.len() as u64);
        header.set_cksum();
        archive.append(&header, out_data.as_slice())?;
    }
    Ok(hashes)
}
