use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::os::unix::fs::OpenOptionsExt;
use tar::Header as TarHeader;
use tar::Builder as TarBuilder;
use tar::EntryType;

use config::Config;
use try::{failed, Try};


const CHMOD_FILE:       u32 = 420;
const CHMOD_BIN_OR_DIR: u32 = 493;

/// Generates the uncompressed control.tar archive
pub fn generate_archive(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: &u64) {
    copy_files(archive, options, time);
    generate_copyright(archive, options, time);
}

/// Generates the copyright file from the license file and adds that to the tar archive.
fn generate_copyright(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: &u64) {
    let mut copyright: Vec<u8> = Vec::new();
    write!(&mut copyright, "Upstream Name: {}\n", options.name).unwrap();
    write!(&mut copyright, "Source: {}\n", options.repository).unwrap();
    write!(&mut copyright, "Copyright: {}\n", options.copyright).unwrap();
    write!(&mut copyright, "License: {}\n", options.license).unwrap();
    options.license_file.get(0)
        // Fail if the path cannot be found and report that the license file argument is missing.
        .map_or_else(|| failed("cargo-deb: missing license file argument"), |path| {
            // Now we need to obtain the amount of lines to skip at the top of the file.
            let lines_to_skip = options.license_file.get(1)
                // If no argument is given, or if the argument is not a number, return 0.
                .map_or(0, |x| x.parse::<usize>().unwrap_or(0));
            // Now we need to attempt to open the file.
            let mut file = fs::File::open(path).try("cargo-deb: license file could not be opened");
            // The capacity of the file can be obtained from the metadata.
            let capacity = file.metadata().map(|x| x.len()).unwrap_or(0) as usize;
            // We are going to store the contents of the license file in a single string with the size of file.
            let mut license_string = String::with_capacity(capacity);
            // Attempt to read the contents of the license file into the license string.
            file.read_to_string(&mut license_string).try("cargo-deb: error reading license file");
            // Skip the first `A` number of lines and then iterate each line after that.
            for line in license_string.lines().skip(lines_to_skip) {
                // If the line is empty, write a dot, else write the line.
                if line.is_empty() {
                    copyright.write(b".\n").unwrap();
                } else {
                    write!(&mut copyright, "{}\n", line.trim()).unwrap();
                }
            }
        });

    // Write a copy to the disk for the sake of obtaining a md5sum for the control archive.
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).mode(CHMOD_FILE)
        .open("target/debian/copyright").unwrap_or_else(|err| {
            failed(format!("cargo-deb: unable to open copyright file for writing: {}", err.to_string()));
        });
    file.write_all(copyright.as_slice()).try("cargo-deb: unable to write copyright file to disk");
    let target = String::from("./usr/share/doc/") + &options.name + "/";

    for dir in &[".", "./usr/", "./usr/share/", "./usr/share/doc/", target.as_str()] {
        let mut header = TarHeader::new_gnu();
        header.set_mtime(*time);
        header.set_size(0);
        header.set_mode(CHMOD_BIN_OR_DIR);
        header.set_path(&dir).unwrap();
        header.set_entry_type(EntryType::Directory);
        header.set_cksum();
        archive.append(&header, &mut io::empty()).unwrap();
    }

    // Now add a copy to the archive
    let mut header = TarHeader::new_gnu();
    header.set_mtime(*time);
    header.set_path(&(target + "copyright")).unwrap();
    header.set_size(copyright.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, copyright.as_slice()).try("cargo-deb: unable to append copyright");
}

/// Copies all the files to be packaged into the tar archive.
fn copy_files(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: &u64) {
    let mut added_directories: Vec<String> = Vec::new();
    for asset in &options.assets {
        // Collect the source and target paths
        let origin = asset.get(0).try("cargo-deb: unable to get asset's path");
        let mut target = String::from("./") + asset.get(1).try("cargo-deb: unable to get asset's target");
        let chmod = asset.get(2).map(|x| u32::from_str_radix(x, 8).unwrap())
            .try("cargo-deb: unable to get chmod argument");
        if target.chars().next().unwrap() == '/' { target.remove(0); }
        if target.chars().last().unwrap() == '/' {
            target.push_str(Path::new(origin).file_name().unwrap().to_str().unwrap());
        }

        // Collect a list of directories
        let directories = target.char_indices()
            .filter(|&(_, character)| character == '/')
            .map(|(id, _)| String::from(&target[0..id+1]))
            .collect::<Vec<String>>();

        // Create all of the intermediary directories in the archive before adding the file
        for directory in directories {
            if !added_directories.iter().any(|x| x == &directory) {
                added_directories.push(directory.clone());
                let mut header = TarHeader::new_gnu();
                header.set_mtime(*time);
                header.set_size(0);
                header.set_mode(CHMOD_BIN_OR_DIR);
                header.set_path(&directory).unwrap();
                header.set_entry_type(EntryType::Directory);
                header.set_cksum();
                archive.append(&header, &mut io::empty()).unwrap();
            }
        }

        // Add the file to the archive
        let mut file = fs::File::open(&origin).try("cargo-deb: unable to open file");
        let capacity = file.metadata().ok().map_or(0, |x| x.len()) as usize;
        let mut out_data: Vec<u8> = Vec::with_capacity(capacity);
        file.read_to_end(&mut out_data).try("cargo-deb: unable to read asset's data");
        let mut header = TarHeader::new_gnu();
        header.set_mtime(*time);
        header.set_path(&target).unwrap();
        header.set_mode(chmod);
        header.set_size(capacity as u64);
        header.set_cksum();
        archive.append(&header, out_data.as_slice()).try("cargo-deb: unable to write data to archive.");
    }
}
