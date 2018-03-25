use std::io;
use std::path::{Path, PathBuf, Component};
use tar;
use tar::Header as TarHeader;
use tar::EntryType;
use std::collections::HashSet;
use error::*;

pub struct Archive {
    added_directories: HashSet<PathBuf>,
    time: u64,
    tar: tar::Builder<Vec<u8>>,
}

impl Archive {
    pub fn new(time: u64) -> Self {
        Self {
            added_directories: HashSet::new(),
            time,
            tar: tar::Builder::new(Vec::new())
        }
    }

    fn directory(&mut self, path: &Path) -> io::Result<()> {
        let mut header = TarHeader::new_gnu();
        header.set_mtime(self.time);
        header.set_size(0);
        header.set_mode(0o755);
        // Lintian insists on dir paths ending with /, which Rust doesn't
        let mut path_str = path.to_string_lossy().to_string();
        if !path_str.ends_with('/') {
            path_str += "/";
        }
        header.set_path(&path_str)?;
        header.set_entry_type(EntryType::Directory);
        header.set_cksum();
        self.tar.append(&header, &mut io::empty())
    }

    fn add_parent_directories(&mut self, path: &Path) -> CDResult<()> {
        // Append each of the directories found in the file's pathname to the archive before adding the file
        // For each directory pathname found, attempt to add it to the list of directories
        let asset_relative_dir = Path::new(".").join(path.parent().ok_or("invalid asset")?);
        let mut directory = PathBuf::new();
        for comp in asset_relative_dir.components() {
            match comp {
                Component::CurDir if !::TAR_REJECTS_CUR_DIR => directory.push("."),
                Component::Normal(c) => directory.push(c),
                _ => continue,
            }
            if !self.added_directories.contains(&directory) {
                self.added_directories.insert(directory.clone());
                self.directory(&directory)?;
            }
        }
        Ok(())
    }

    pub fn file<P: AsRef<Path>>(&mut self, path: P, out_data: &[u8], chmod: u32) -> CDResult<()> {
        self.add_parent_directories(path.as_ref())?;

        let mut header = TarHeader::new_gnu();
        header.set_mtime(self.time);
        header.set_path(path)?;
        header.set_mode(chmod);
        header.set_size(out_data.len() as u64);
        header.set_cksum();
        self.tar.append(&header, out_data)?;
        Ok(())
    }

    pub fn into_inner(self) -> io::Result<Vec<u8>> {
        self.tar.into_inner()
    }
}
