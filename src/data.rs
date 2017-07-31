use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::os::unix::fs::OpenOptionsExt;
use itertools::Itertools;
use tar::Header as TarHeader;
use tar::Builder as TarBuilder;
use tar::EntryType;
use md5::Digest;
use md5;

use config::Config;
use try::{failed, Try};
use std::collections::HashMap;

const CHMOD_FILE:       u32 = 420;
const CHMOD_BIN_OR_DIR: u32 = 493;

/// Generates the uncompressed control.tar archive
pub fn generate_archive(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) -> HashMap<String, Digest>{
    let copy_hashes = copy_files(archive, options, time);
    generate_copyright(archive, options, time);
    copy_hashes
}

/// Generates the copyright file from the license file and adds that to the tar archive.
fn generate_copyright(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) {
    let mut copyright: Vec<u8> = Vec::new();
    write!(&mut copyright, "Upstream Name: {}\n", options.name).unwrap();
    write!(&mut copyright, "Source: {}\n", options.repository).unwrap();
    write!(&mut copyright, "Copyright: {}\n", options.copyright).unwrap();
    write!(&mut copyright, "License: {}\n", options.license).unwrap();
    options.license_file.as_ref()
        // Fail if the path cannot be found and report that the license file argument is missing.
        .map(|path| {
            // Now we need to attempt to open the file.
            let mut file = fs::File::open(path).try("license file could not be opened");
            let mut license_string = String::new();
            file.read_to_string(&mut license_string).try("error reading license file");
            // Skip the first `A` number of lines and then iterate each line after that.
            for line in license_string.lines().skip(options.license_file_skip_lines) {
                // If the line is empty, write a dot, else write the line.
                if line.is_empty() {
                    copyright.write(b".\n").unwrap();
                } else {
                    copyright.write(line.trim().as_bytes()).unwrap();
                    copyright.write(b"\n").unwrap();
                }
            }
        });

    // Write a copy to the disk for the sake of obtaining a md5sum for the control archive.
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).mode(CHMOD_FILE)
        .open("target/debian/copyright").unwrap_or_else(|err| {
            failed(format!("unable to open copyright file for writing: {}", err.to_string()));
        });
    file.write_all(copyright.as_slice()).try("unable to write copyright file to disk");
    let target = String::from("./usr/share/doc/") + &options.name + "/";

    for dir in &[".", "./usr/", "./usr/share/", "./usr/share/doc/", target.as_str()] {
        let mut header = TarHeader::new_gnu();
        header.set_mtime(time);
        header.set_size(0);
        header.set_mode(CHMOD_BIN_OR_DIR);
        header.set_path(&dir).unwrap();
        header.set_entry_type(EntryType::Directory);
        header.set_cksum();
        archive.append(&header, &mut io::empty()).unwrap();
    }

    // Now add a copy to the archive
    let mut header = TarHeader::new_gnu();
    header.set_mtime(time);
    header.set_path(&(target + "copyright")).unwrap();
    header.set_size(copyright.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, copyright.as_slice()).try("unable to append copyright");
}

/// Copies all the files to be packaged into the tar archive.
/// Returns MD5 hashes of files copied
fn copy_files(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) -> HashMap<String, Digest> {
    let mut hashes = HashMap::new();
    let mut added_directories: Vec<String> = Vec::new();
    for asset in &options.assets {
        // Collect the source and target paths
        let mut target = String::from("./") + &asset.target_path;
        if target.chars().next().unwrap() == '/' { target.remove(0); }
        if target.chars().last().unwrap() == '/' {
            target.push_str(Path::new(&asset.source_file).file_name().unwrap().to_str().unwrap());
        }

        // Append each of the directories found in the file's pathname to the archive before adding the file
        target.char_indices()
            // Exclusively search for `/` characters only
            .filter(|&(_, character)| character == '/')
            // Use the indexes of the `/` characters to collect a list of directory pathnames
            .map(|(id, _)| &target[0..id+1])
            // For each directory pathname found, attempt to add it to the list of directories
            .foreach(|directory| {
                if !added_directories.iter().any(|x| x.as_str() == directory) {
                    added_directories.push(directory.to_owned());
                    let mut header = TarHeader::new_gnu();
                    header.set_mtime(time);
                    header.set_size(0);
                    header.set_mode(CHMOD_BIN_OR_DIR);
                    header.set_path(&directory).unwrap();
                    header.set_entry_type(EntryType::Directory);
                    header.set_cksum();
                    archive.append(&header, &mut io::empty()).unwrap();
                }
            });

        // Add the file to the archive
        let mut file = fs::File::open(&asset.source_file).try("unable to open file");
        let mut out_data = Vec::new();
        file.read_to_end(&mut out_data).try("unable to read asset's data");

        hashes.insert(asset.source_file.clone(), md5::compute(&out_data));

        let mut header = TarHeader::new_gnu();
        header.set_mtime(time);
        header.set_path(&target).unwrap();
        header.set_mode(asset.chmod);
        header.set_size(out_data.len() as u64);
        header.set_cksum();
        archive.append(&header, out_data.as_slice()).try("unable to write data to archive.");
    }
    hashes
}
