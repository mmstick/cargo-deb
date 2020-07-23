use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Get the filename from a path. Intended to be erplaced when testing.
pub(crate) fn fname_from_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}

/// Copy a file from `from` to `to`. Intended to be replaced when testing.
pub(crate) fn copy_file(from: &Path, to: &Path) -> std::io::Result<u64> {
    std::fs::copy(&from, &to)
}

/// Create a HashMap from one or more key => value pairs in a single statement.
/// 
/// # Provenance
/// 
/// From: https://stackoverflow.com/a/27582993
/// 
/// # Usage
/// 
/// ```
/// let names = map!{ 1 => "one", 2 => "two" };
/// ```
#[macro_export]
macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

/// A trait for returning a String containing items separated by the given
/// separator.
pub(crate) trait MyJoin {
    fn join(&self, sep: &str) -> String;
}

/// Returns a String containing the hash set items joined together by the given
/// separator.
impl MyJoin for HashSet<String> {
    fn join(&self, sep: &str) -> String {
        let mut v = Vec::<&str>::new();
        for item in self.iter() {
            v.push(item.as_str());
        }
        v.join(sep)
    }
}

/// Return Some(path) to the first directory in the `search_dirs` array that
/// contains an immediate child file with name `filename`, if found, else None.
pub(crate) fn find_first(search_dirs: &[PathBuf], filename: &str) -> Option<PathBuf> {
    search_dirs.iter().find_map(|dir| {
        let abs_path = dir.join(filename);
        match abs_path.exists() {
            true => Some(abs_path),
            false => None
        }
    })
}
