use file;
use std::io::{self, Write};
use std::fs;
use std::path::Path;
use config::Config;
use try::Try;
use tar::Builder as TarBuilder;
use tar::Header as TarHeader;
use tar::EntryType;
use md5::Digest;
use md5;
use std::collections::HashMap;
use error::*;

const CHMOD_FILE:       u32 = 420;
const CHMOD_BIN_OR_DIR: u32 = 493;
const SCRIPTS: 		[&str; 4] = ["preinst", "postinst", "prerm", "postrm"];

/// Generates the uncompressed control.tar archive
pub fn generate_archive(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64, asset_hashes: HashMap<String, Digest>) -> CDResult<()> {
    initialize_control(archive, time)?;
    generate_md5sums(archive, options, time, asset_hashes)?;
    generate_control(archive, options, time)?;
    if let Some(ref files) = options.conf_files {
        generate_conf_files(archive, files, time)?;
    }
    generate_scripts(archive, options);
    Ok(())
}

/// Creates the initial hidden directory where all the files are stored.
fn initialize_control(archive: &mut TarBuilder<Vec<u8>>, time: u64) -> io::Result<()> {
    if ::TAR_REJECTS_CUR_DIR {
        return Ok(());
    }
    let mut header = TarHeader::new_gnu();
    header.set_mtime(time);
    header.set_size(0);
    header.set_mode(CHMOD_BIN_OR_DIR);
    header.set_path("./")?;
    header.set_entry_type(EntryType::Directory);
    header.set_cksum();
    archive.append(&header, &mut io::empty())
}

/// Append all files that reside in the `maintainer_scripts` path to the archive
fn generate_scripts(archive: &mut TarBuilder<Vec<u8>>, option: &Config) {
    if let Some(ref maintainer_scripts) = option.maintainer_scripts {
        for script in &SCRIPTS {
            if let Ok(mut file) = fs::File::open(maintainer_scripts.join(script)) {
                archive.append_file(script, &mut file)
                    .try("failed to add maintainer script to control");
            }
        }
    }
}

/// Creates the md5sums file which contains a list of all contained files and the md5sums of each.
fn generate_md5sums(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64, asset_hashes: HashMap<String, md5::Digest>) -> CDResult<()> {
    let mut md5sums: Vec<u8> = Vec::new();

    // Collect md5sums from each asset in the archive.
    for asset in &options.assets {
        let mut target = asset.target_path.clone();
        if target.chars().next().unwrap() == '/' { target.remove(0); }

        let mut hash = Vec::new();
        write!(hash, "{:x}", asset_hashes[&asset.source_file])?;
        hash.write(b"  ")?;

        let target_is_dir = target.chars().last().unwrap() == '/';
        if target_is_dir {
            let filename = Path::new(&asset.source_file).file_name().unwrap().to_str().unwrap();
            hash.write(target.as_bytes())?;
            hash.write(filename.as_bytes())?;
        } else {
            hash.write(asset.target_path.as_bytes())?;
        }
        hash.write(&[b'\n'])?;
        md5sums.append(&mut hash);
    }

    // Obtain the md5sum of the copyright file
    let copyright_file = file::get("target/debian/copyright").try("unable to open target/debian/copyright");

    let mut hash = Vec::new();
    write!(hash, "{:x}", md5::compute(&copyright_file))?;
    hash.write(b"  ")?;

    let path = String::from("usr/share/doc/") + &options.name + "/copyright";
    hash.write(path.as_bytes())?;
    md5sums.append(&mut hash);
    md5sums.push(b'\n');

    // We can now exterminate the copyright file as it has outlived it's usefulness.
    fs::remove_file("target/debian/copyright").try("copyright file doesn't exist.");

    // Write the data to the archive
    let mut header = TarHeader::new_gnu();
    header.set_mtime(time);
    header.set_path("./md5sums")?;
    header.set_size(md5sums.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, md5sums.as_slice())?;
    Ok(())
}

/// Generates the control file that obtains all the important information about the package.
fn generate_control(archive: &mut TarBuilder<Vec<u8>>, options: &Config, time: u64) -> io::Result<()> {
    // Create and return the handle to the control file with write access.
    let mut control: Vec<u8> = Vec::with_capacity(1024);

    // Write all of the lines required by the control file.
    write!(&mut control, "Package: {}\n", options.name)?;
    write!(&mut control, "Version: {}\n", options.version)?;
    write!(&mut control, "Architecture: {}\n", options.architecture)?;
    write!(&mut control, "Vcs-{}: {}\n", options.repository_type(), options.repository)?;
    if let Some(ref homepage) = options.homepage.as_ref().or(options.documentation.as_ref()) {
        write!(&mut control, "Homepage: {}\n", homepage)?;
    }
    if let Some(ref section) = options.section {
        write!(&mut control, "Section: {}\n", section)?;
    }
    write!(&mut control, "Priority: {}\n", options.priority)?;
    control.write(b"Standards-Version: 3.9.4\n")?;
    write!(&mut control, "Maintainer: {}\n", options.maintainer)?;
    write!(&mut control, "Depends: {}\n", options.get_dependencies())?;
    write!(&mut control, "Description: {}\n", options.description)?;

    // Write each of the lines that were collected from the extended_description to the file.
    for line in &options.extended_description {
        write!(&mut control, " {}\n", line)?;
    }
    control.push(10);

    // Add the control file to the tar archive.
    let mut header = TarHeader::new_gnu();
    header.set_mtime(time);
    header.set_path("./control")?;
    header.set_size(control.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, control.as_slice())
}

/// If configuration files are required, the conffiles file will be created.
fn generate_conf_files(archive: &mut TarBuilder<Vec<u8>>, files: &str, time: u64) -> io::Result<()> {
    let mut data: Vec<u8> = Vec::with_capacity(files.chars().count() + 1);
    data.write(files.as_bytes())?;
    data.push(10);
    let mut header = TarHeader::new_gnu();
    header.set_mtime(time);
    header.set_path("./conffiles")?;
    header.set_size(data.len() as u64);
    header.set_mode(CHMOD_FILE);
    header.set_cksum();
    archive.append(&header, data.as_slice())?;
    Ok(())
}
