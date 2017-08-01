use std::fs;
use std::io::Write;
use zopfli::{self, Options, Format};
use lzma;
use error::*;

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: Vec<u8>, path: &str) -> CDResult<()> {
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).open(path)
        .map_err(|why| CargoDebError::IoFile(why, path.to_owned()))?;

    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);

    zopfli::compress(&Options::default(), &Format::Gzip, &data, &mut compressed)?;

    // If the compression succeeded, attempt to write the file to disk.
    file.write(&compressed[..])
        .map_err(|why| CargoDebError::IoFile(why, path.to_owned()))?;
    Ok(())
}

/// Compresses data using the system's xz library, which requires `liblzma-dev` to be installed
pub fn xz(data: Vec<u8>, path: &str) -> CDResult<()> {
    let mut file = fs::OpenOptions::new().create(true).write(true).truncate(true).open(path)
        .map_err(|why| CargoDebError::IoFile(why, path.to_owned()))?;

    let compressed = lzma::compress(&data, 9)?;

    file.write(&compressed[..]).map(|_| ())
        .map_err(|why| CargoDebError::IoFile(why, path.to_owned()))?;

    Ok(())
}

