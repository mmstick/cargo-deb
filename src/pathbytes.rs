#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub trait AsBytes {
    fn as_bytes(&self) -> &[u8];
}

impl AsBytes for Path {
    #[cfg(not(unix))]
    fn as_bytes(&self) -> &[u8] {
        self.to_str().expect("Paths must be valid Unicode").as_bytes()
    }
    #[cfg(unix)]
    fn as_bytes(&self) -> &[u8] {
        self.as_os_str().as_bytes()
    }
}
