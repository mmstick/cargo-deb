use std::collections::HashSet;
use std::path::Path;

/// Get the filename from a path. Intended to be replaced when testing.
pub(crate) fn fname_from_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}

/// Create a HashMap from one or more key => value pairs in a single statement.
/// 
/// # Provenance
/// 
/// From: https://stackoverflow.com/a/27582993
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