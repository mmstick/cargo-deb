use std::fs;
use std::io::Write;
use zopfli::{self, Options, Format};
#[cfg(feature = "lzma")]
use lzma;
use error::*;

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: Vec<u8>, base_path: &str) -> CDResult<String> {
    let full_path = format!("{}.gz", base_path);
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).open(&full_path)
        .map_err(|why| CargoDebError::IoFile(why, full_path.clone()))?;

    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);

    zopfli::compress(&Options::default(), &Format::Gzip, &data, &mut compressed)?;

    // If the compression succeeded, attempt to write the file to disk.
    file.write(&compressed[..])
        .map_err(|why| CargoDebError::IoFile(why, full_path.clone()))?;
    Ok(full_path)
}

/// Compresses data using the system's xz library, which requires `liblzma-dev` to be installed
#[cfg(feature = "lzma")]
pub fn xz_or_gz(data: Vec<u8>, base_path: &str) -> CDResult<String> {
    let full_path = format!("{}.xz", base_path);
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).open(&full_path)
        .map_err(|why| CargoDebError::IoFile(why, full_path.clone()))?;

    let compressed = lzma::compress(&data, 9)?;

    file.write(&compressed[..]).map(|_| ())
        .map_err(|why| CargoDebError::IoFile(why, full_path.clone()))?;

    Ok(full_path)
}

#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(data: Vec<u8>, base_path: &str) -> CDResult<String> {
    gz(data, base_path)
}

