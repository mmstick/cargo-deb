use std::io::Read;
use std::ops;

use crate::error::*;

pub enum Compressed {
    Gz(Vec<u8>),
    Xz(Vec<u8>),
}

impl ops::Deref for Compressed {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Gz(data) => &data,
            Self::Xz(data) => &data,
        }
    }
}

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: &[u8]) -> CDResult<Vec<u8>> {
    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);

    let level = flate2::Compression::default();
    flate2::bufread::GzEncoder::new(data, level).read_to_end(&mut compressed)?;
    compressed.shrink_to_fit();

    Ok(compressed)
}

#[cfg(feature = "lzma")]
fn xz(data: &[u8]) -> CDResult<Vec<u8>> {
    use xz2::bufread::XzEncoder;

    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);
    // Compression level 6 is a good trade off between size and [ridiculously] long compression time
    XzEncoder::new(data, 6).read_to_end(&mut compressed)?;
    compressed.shrink_to_fit();

    Ok(compressed)
}

/// Compresses data using the xz2 library
#[cfg(feature = "lzma")]
pub fn xz_or_gz(data: &[u8], fast: bool) -> CDResult<Compressed> {
    if fast {
        gz(data).map(Compressed::Gz)
    } else {
        xz(data).map(Compressed::Xz)
    }
}

#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(data: &[u8], _fast: bool) -> CDResult<Compressed> {
    gz(data).map(Compressed::Gz)
}
