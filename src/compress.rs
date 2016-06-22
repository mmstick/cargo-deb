use std::fs;
use std::io::Write;
use zopfli;
use lzma;
use try::{failed, Try};

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: Vec<u8>, path: &str) {
    fs::OpenOptions::new().create(true).write(true).truncate(true).open(path).ok()
        .map_or_else(|| failed(format!("cargo-deb: unable to create {}", path)), |mut file| {
            let mut compressed: Vec<u8> = Vec::new();
            let options = zopfli::Options::default();
            let format = zopfli::Format::Gzip;
            zopfli::compress(&options, &format, &data, &mut compressed)
                .try("cargo-deb: error with zopfli compression");
            file.write(&compressed[..])
                .try(&format!("cargo-deb: unable to write to {}", path));
        });
}

/// Compresses data using the system xz library
pub fn xz(data: Vec<u8>, path: &str) {
    fs::OpenOptions::new().create(true).write(true).truncate(true).open(path).ok()
        .map_or_else(|| failed(format!("cargo-deb: unable to create {}", path)), |mut file| {
            let data = lzma::compress(&data, 9).try("cargo-deb: failed to compress archive with xz");
            file.write(&data[..]).try(&format!("cargo-deb: unable to write to {}", path));
        });
}
