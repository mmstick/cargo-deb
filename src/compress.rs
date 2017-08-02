use std::fs;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::io::Write;
use zopfli::{self, Options, Format};
#[cfg(feature = "lzma")]
use lzma;
use error::*;

fn append(path: &Path, ext: &str) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(OsStr::new(ext));
    path.with_file_name(name)
}

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: Vec<u8>, base_path: &Path) -> CDResult<PathBuf> {
    let full_path = append(base_path, ".gz");
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).open(&full_path)
        .map_err(|why| CargoDebError::IoFile(why, full_path.display().to_string()))?;

    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);

    zopfli::compress(&Options::default(), &Format::Gzip, &data, &mut compressed)?;

    // If the compression succeeded, attempt to write the file to disk.
    file.write(&compressed[..])
        .map_err(|why| CargoDebError::IoFile(why, full_path.display().to_string()))?;
    Ok(full_path)
}

/// Compresses data using the system's xz library, which requires `liblzma-dev` to be installed
#[cfg(feature = "lzma")]
pub fn xz_or_gz(data: Vec<u8>, base_path: &Path) -> CDResult<PathBuf> {
    let full_path = append(base_path, ".xz");
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).open(&full_path)
        .map_err(|why| CargoDebError::IoFile(why, full_path.display().to_string()))?;

    let compressed = lzma::compress(&data, 9)?;

    file.write(&compressed[..]).map(|_| ())
        .map_err(|why| CargoDebError::IoFile(why, full_path.display().to_string()))?;

    Ok(full_path)
}

#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(data: Vec<u8>, base_path: &Path) -> CDResult<String> {
    gz(data, base_path)
}

