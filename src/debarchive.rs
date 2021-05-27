use crate::error::CDResult;
use crate::manifest::Config;
use crate::pathbytes::*;
use ar::{Builder, Header};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

const DEFAULT_SEPARATOR: char = '_';

pub struct DebArchive {
    out_abspath: PathBuf,
    prefix: PathBuf,
    ar_builder: Builder<File>,
}

impl DebArchive {
    pub fn new(config: &Config) -> CDResult<Self> {
        let out_filename = format!("{}{sep}{}{sep}{}.deb", config.deb_name, config.deb_version, config.architecture,
            sep = config.deb_name_separator.unwrap_or(DEFAULT_SEPARATOR)
        );
        let prefix = config.deb_temp_dir();
        let out_abspath = config.deb_output_path(&out_filename);
        {
            let deb_dir = out_abspath.parent().ok_or("invalid dir")?;
            let _ = fs::create_dir_all(deb_dir);
        }
        let ar_builder = Builder::new(File::create(&out_abspath)?);

        Ok(DebArchive {
            out_abspath,
            prefix,
            ar_builder,
        })
    }

    pub(crate) fn filename_glob(config: &Config) -> String {
        format!("{}{sep}*{sep}{}.deb", config.deb_name, config.architecture,
            sep = config.deb_name_separator.unwrap_or(DEFAULT_SEPARATOR)
        )
    }

    pub fn add_path(&mut self, path: &Path) -> CDResult<()> {
        let dest_path = path.strip_prefix(&self.prefix).map_err(|_| "invalid path")?;
        let mut file = File::open(&path)?;
        self.ar_builder.append_file(&dest_path.as_unix_path(), &mut file)?;
        Ok(())
    }

    pub fn add_data(&mut self, dest_path: &str, mtime_timestamp: u64, data: &[u8]) -> CDResult<()> {
        let mut header = Header::new(dest_path.as_bytes().to_owned(), data.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(mtime_timestamp);
        header.set_uid(0);
        header.set_gid(0);
        self.ar_builder.append(&header, data)?;
        Ok(())
    }

    pub fn finish(self) -> CDResult<PathBuf> {
        Ok(self.out_abspath)
    }
}
