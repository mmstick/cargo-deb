use std::collections::BTreeSet;
use std::path::Path;

/// Get the filename from a path. Intended to be replaced when testing.
pub(crate) fn fname_from_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into()
}

/// Create a HashMap from one or more key => value pairs in a single statement.
/// 
/// # Usage
/// 
/// Any types supported by HashMap for keys and values are supported:
/// ```
/// let mut one = std::collections::HashMap::new();
/// one.insert(1, 'a');
/// assert_eq!(one, map!{ 1 => 'a' });
///
/// let mut two = std::collections::HashMap::new();
/// two.insert("a", 1);
/// two.insert("b", 2);
/// assert_eq!(two, map!{ "a" => 1, "b" => 2 });
/// ```
/// 
/// Empty maps are not supported, attempting to create one will fail to compile:
/// ```compile_fail
/// let empty = std::collections::HashMap::new();
/// assert_eq!(empty, map!{ });
/// ```
/// 
/// # Provenance
/// 
/// From: https://stackoverflow.com/a/27582993
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
/// 
/// # Usage
/// 
/// ```text
/// let two: BTreeSet<String> = vec!["a", "b"].into_iter().map(|s| s.to_owned()).collect();
/// assert_eq!("ab", two.join(""));
/// assert_eq!("a,b", two.join(","));
/// ```
impl MyJoin for BTreeSet<String> {
    fn join(&self, sep: &str) -> String {
        self.iter().map(|item| item.as_str()).collect::<Vec<&str>>().join(sep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_macro() {
        let mut one = std::collections::HashMap::new();
        one.insert(1, 'a');
        assert_eq!(one, map!{ 1 => 'a' });

        let mut two = std::collections::HashMap::new();
        two.insert("a", 1);
        two.insert("b", 2);
        assert_eq!(two, map!{ "a" => 1, "b" => 2 });
    }

    #[test]
    fn btreeset_join() {
        let empty: BTreeSet<String> = vec![].into_iter().collect();
        assert_eq!("", empty.join(""));
        assert_eq!("", empty.join(","));

        let one: BTreeSet<String> = vec!["a"].into_iter().map(|s| s.to_owned()).collect();
        assert_eq!("a", one.join(""));
        assert_eq!("a", one.join(","));

        let two: BTreeSet<String> = vec!["a", "b"].into_iter().map(|s| s.to_owned()).collect();
        assert_eq!("ab", two.join(""));
        assert_eq!("a,b", two.join(","));
    }
}