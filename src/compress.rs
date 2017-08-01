use std::fs;
use std::io::{self, Write};
use zopfli::{self, Options, Format};
use lzma;

#[derive(Debug)]
pub enum Archive { Data, Control }

#[derive(Debug)]
pub enum CompressErr {
    UnableToCreatePath(io::Error),
    Compression(String),
    Write(Archive, io::Error)
}

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
pub fn gz(data: Vec<u8>, path: &str) -> Result<(), CompressErr> {
    fs::OpenOptions::new().create(true).write(true).truncate(true).open(path)
        // If the directory cannot be created, return an error
        .map_err(|why| CompressErr::UnableToCreatePath(why)).and_then(|mut file| {
            // Compressed data is typically half to a third the original size
            let mut compressed: Vec<u8> = Vec::with_capacity(data.len() >> 1);
            // Attempt to compress the data with the Zopfli compression algorithm
            zopfli::compress(&Options::default(), &Format::Gzip, &data, &mut compressed)
                // If an error occurred in compression, create an error
                .map_err(|why| CompressErr::Compression(why.to_string())).and_then(|_| {
                    // If the compression succeeded, attempt to write the file to disk.
                    file.write(&compressed[..]).map(|_| ())
                        // If an error occured, return the error.
                        .map_err(|why| CompressErr::Write(Archive::Data, why))
                })
        })
}

/// Compresses data using the system's xz library, which requires `liblzma-dev` to be installed
pub fn xz(data: Vec<u8>, path: &str) -> Result<(), CompressErr> {
    fs::OpenOptions::new().create(true).write(true).truncate(true).open(path)
        // If the directory cannot be created, return an error
        .map_err(|why| CompressErr::UnableToCreatePath(why)).and_then(|mut file| {
            // Attempt to compress the data with the LZMA compression algorithm
            lzma::compress(&data, 9)
                // If an error occurred in compression, create an error
                .map_err(|why| CompressErr::Compression(why.to_string())).and_then(|compressed| {
                    // If the compression succeeded, attempt to write the file to disk.
                    file.write(&compressed[..]).map(|_| ())
                        // If an error occured, return the error.
                        .map_err(|why| CompressErr::Write(Archive::Control, why))
                })
        })
}

