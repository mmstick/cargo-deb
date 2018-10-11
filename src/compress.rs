use zopfli::{self, Format, Options};

use error::*;

pub enum Compressed {
    Gz(Vec<u8>),
    Xz(Vec<u8>),
}

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: &[u8]) -> CDResult<Vec<u8>> {
    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);
    zopfli::compress(&Options::default(), &Format::Gzip, data, &mut compressed)?;

    Ok(compressed)
}

/// Compresses data using the xz2 library
#[cfg(feature = "lzma")]
pub fn xz_or_gz(data: &[u8]) -> CDResult<Compressed> {
    use std::io::prelude::*;
    use xz2::read::XzEncoder;

    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);
    let mut compressor = XzEncoder::new(data, 9);
    compressor.read_to_end(&mut compressed)?;

    Ok(Compressed::Xz(compressed))
}

#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(data: &[u8]) -> CDResult<Vec<u8>> {
    gz(data, base_path).map(Compressed::Gz)
}
