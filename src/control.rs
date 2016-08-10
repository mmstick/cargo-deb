use std::io::{self, Write};
use std::fs;
use std::path::Path;
use std::process::Command;
use config::Config;
use try::{failed, Try};
use tar::Builder as TarBuilder;
use tar::Header as TarHeader;
use tar::EntryType;

const CHMOD_FILE:       u32 = 420;
const CHMOD_BIN_OR_DIR: u32 = 493;

/// Generates the uncompressed control.tar archive
pub fn generate_archive(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: &u64) {
    initialize_control(archive, time);
    generate_md5sums(archive, options, time);
    generate_control(archive, options, time);
    generate_conf_files(archive, options.conf_files.as_ref(), time);
}

/// Creates the initial hidden directory where all the files are stored.
fn initialize_control(archive: &mut TarBuilder<Vec<u8>>, time: &u64) {
    let mut header = TarHeader::new_gnu();
    header.set_mtime(*time);
    header.set_size(0);
    header.set_mode(CHMOD_BIN_OR_DIR);
    header.set_path("./").unwrap();
    header.set_entry_type(EntryType::Directory);
    header.set_cksum();
    archive.append(&header, &mut io::empty()).unwrap();
}

/// Creates the md5sums file which contains a list of all contained files and the md5sums of each.
fn generate_md5sums(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: &u64) {
    let mut md5sums: Vec<u8> = Vec::new();

    // Collect md5sums from each asset in the archive.
    for asset in &options.assets {
        let origin = asset.get(0).try("cargo-deb: unable to get asset's path");
        let mut target: String = asset.get(1).try("cargo-deb: unable to get asset's target").clone();
        if target.chars().next().unwrap() == '/' { target.remove(0); }
        let target_is_dir = target.chars().last().unwrap() == '/';
        Command::new("md5sum").arg(&origin).output().ok()
            .map_or_else(|| failed("cargo-deb: could not get output of md5sum"), |x| {
                let mut hash = x.stdout.iter().take_while(|&&x| x != b' ').cloned().collect::<Vec<u8>>();
                hash.write(b"  ").unwrap();
                if target_is_dir {
                    let filename = Path::new(origin).file_name().unwrap().to_str().unwrap();
                    write!(&mut hash, "{}{}", target, filename).unwrap();
                } else {
                    hash.write(asset.get(1).unwrap().as_bytes()).unwrap();
                }
                hash.write(&[b'\n']).unwrap();
                md5sums.append(&mut hash);
            });
    }

    // Obtain the md5sum of the copyright file
    Command::new("md5sum").arg("target/debian/copyright").output().ok()
        .map_or_else(|| failed("cargo-deb: could not get output of md5sum"), |x| {
            let mut hash = x.stdout.iter().take_while(|&&x| x != b' ').cloned().collect::<Vec<u8>>();
            let path = String::from("usr/share/doc/") + &options.name + "/copyright";
            hash.write(b"  ").unwrap();
            hash.write(path.as_bytes()).unwrap();
            md5sums.append(&mut hash);
            md5sums.push(10);
        });

    // We can now exterminate the copyright file as it has outlived it's usefulness.
    fs::remove_file("target/debian/copyright").try("cargo-deb: copyright file doesn't exist.");

    // Write the data to the archive
    let mut header = TarHeader::new_gnu();
    header.set_mtime(*time);
    header.set_path("./md5sums").unwrap();
    header.set_size(md5sums.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, md5sums.as_slice()).try("cargo-deb: unable to append md5sums");
}

/// Generates the control file that obtains all the important information about the package.
fn generate_control(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: &u64) {
    // Create and return the handle to the control file with write access.
    let mut control: Vec<u8> = Vec::new();

    // Write all of the lines required by the control file.
    write!(&mut control, "Package: {}\n", options.name).unwrap();
    write!(&mut control, "Version: {}\n", options.version).unwrap();
    write!(&mut control, "Architecture: {}\n", options.architecture).unwrap();
    write!(&mut control, "Repository: {}\n", options.repository).unwrap();
    if let Some(ref homepage) = options.homepage {
        write!(&mut control, "Homepage: {}\n", homepage).unwrap();
    }
    write!(&mut control, "Section: {}\n", options.section).unwrap();
    write!(&mut control, "Priority: {}\n", options.priority).unwrap();
    control.write(b"Standards-Version: 3.9.4\n").unwrap();
    write!(&mut control, "Maintainer: {}\n", options.maintainer).unwrap();
    write!(&mut control, "Depends: {}\n", options.depends).unwrap();
    write!(&mut control, "Description: {}\n", options.description).unwrap();

    // Write each of the lines that were collected from the extended_description to the file.
    for line in &options.extended_description {
        write!(&mut control, " {}\n", line).unwrap();
    }
    control.push(10);

    // Add the control file to the tar archive.
    let mut header = TarHeader::new_gnu();
    header.set_mtime(*time);
    header.set_path("./control").unwrap();
    header.set_size(control.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, control.as_slice()).try("cargo-deb: unable to append control");
}

/// If configuration files are required, the conffiles file will be created.
fn generate_conf_files(archive: &mut TarBuilder<Vec<u8>>, conf_file: Option<&String>, time: &u64) {
    if let Some(files) = conf_file {
        let mut data: Vec<u8> = Vec::new();
        data.write(files.clone().as_bytes()).unwrap();
        data.push(10);
        let mut header = TarHeader::new_gnu();
        header.set_mtime(*time);
        header.set_path("./conffiles").unwrap();
        header.set_size(data.len() as u64);
        header.set_mode(CHMOD_FILE);
        header.set_cksum();
        archive.append(&header, data.as_slice()).try("cargo-deb: unable to append conffiles");
    }
}
