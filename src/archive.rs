use std::io;
use std::path::Path;
use tar;
use tar::Header as TarHeader;
use tar::EntryType;

pub struct Archive {
    time: u64,
    tar: tar::Builder<Vec<u8>>,
}

impl Archive {
    pub fn new(time: u64) -> Self {
        Self {
            time,
            tar: tar::Builder::new(Vec::new())
        }
    }

    pub fn directory<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let mut header = TarHeader::new_gnu();
        header.set_mtime(self.time);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_path(path)?;
        header.set_entry_type(EntryType::Directory);
        header.set_cksum();
        self.tar.append(&header, &mut io::empty())
    }

    pub fn file<P: AsRef<Path>>(&mut self, path: P, out_data: &[u8], chmod: u32) -> io::Result<()> {
        let mut header = TarHeader::new_gnu();
        header.set_mtime(self.time);
        header.set_path(path)?;
        header.set_mode(chmod);
        header.set_size(out_data.len() as u64);
        header.set_cksum();
        self.tar.append(&header, out_data)
    }

    pub fn into_inner(self) -> io::Result<Vec<u8>> {
        self.tar.into_inner()
    }
}
