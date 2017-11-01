use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use zopfli::{self, Options, Format};
#[cfg(feature = "lzma")]
use lzma;
use file;
use error::*;

fn append(path: &Path, ext: &str) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(OsStr::new(ext));
    path.with_file_name(name)
}

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: &[u8], base_path: &Path) -> CDResult<PathBuf> {
    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);
    zopfli::compress(&Options::default(), &Format::Gzip, data, &mut compressed)?;

    // If the compression succeeded, attempt to write the file to disk.
    let full_path = append(base_path, ".gz");
    file::put(&full_path, compressed)
        .map_err(|why| CargoDebError::IoFile("unable to save compressed archive", why, full_path.clone()))?;
    Ok(full_path)
}

/// Compresses data using the system's xz library, which requires `liblzma-dev` to be installed
#[cfg(feature = "lzma")]
pub fn xz_or_gz(data: &[u8], base_path: &Path) -> CDResult<PathBuf> {
    let compressed = lzma::compress(data, 9)?;

    let full_path = append(base_path, ".xz");
    file::put(&full_path, &compressed)
        .map_err(|why| CargoDebError::IoFile("unable to save compressed archive", why, full_path.clone()))?;

    Ok(full_path)
}

#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(data: &[u8], base_path: &Path) -> CDResult<PathBuf> {
    gz(data, base_path)
}
