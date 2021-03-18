use std::ops;

use zopfli::{self, Format, Options};

use crate::error::*;

pub enum Compressed {
    Gz(Vec<u8>),
    Xz(Vec<u8>),
}

impl ops::Deref for Compressed {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Gz(data) | Self::Xz(data) => &data,
        }
    }
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
pub fn xz_or_gz(data: &[u8], fast: bool) -> CDResult<Compressed> {
    use xz2::stream;

    // Compressed data is typically half to a third the original size
    let mut compressed = Vec::with_capacity(data.len() >> 1);

    // Compression level 6 is a good trade off between size and [ridiculously] long compression time
    let mut encoder = stream::MtStreamBuilder::new()
        .threads(num_cpus::get() as u32)
        .preset(if fast { 1 } else { 6 })
        .encoder()
        .map_err(|e| CargoDebError::LzmaCompressionError(e))?;

    encoder
        .process_vec(data, &mut compressed, stream::Action::Finish)
        .map_err(|e| CargoDebError::LzmaCompressionError(e))?;

    compressed.shrink_to_fit();

    Ok(Compressed::Xz(compressed))
}

#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(data: &[u8], _fast: bool) -> CDResult<Compressed> {
    gz(data).map(Compressed::Gz)
}
