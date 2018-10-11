use manifest::Config;
use std::path::PathBuf;
use error::CDResult;
use pathbytes::*;
use std::fs;
use std::fs::File;
use ar::Builder;

/// Uses the ar program to create the final Debian package, at least until a native ar implementation is implemented.
pub fn generate_deb(config: &Config, contents: &[PathBuf]) -> CDResult<PathBuf> {
    let out_filename = format!("{}_{}_{}.deb", config.name, config.version, config.architecture);
    let out_abspath = config.deb_output_path(&out_filename);
    let prefix = config.deb_temp_dir();
    {
        let deb_dir = out_abspath.parent().ok_or("invalid dir")?;

        let _ = fs::create_dir_all(deb_dir);
        let mut ar_builder = Builder::new(File::create(&out_abspath)?);

        for path in contents {
            let dest_path = path.strip_prefix(&prefix).map_err(|_| "invalid path")?;
            let mut file = File::open(&path)?;
            ar_builder.append_file(&dest_path.as_unix_path(), &mut file)?;
        }
    }
    Ok(out_abspath)
}
