use std::collections::BTreeSet;
use std::path::Path;

/// Get the filename from a path. Intended to be replaced when testing.
pub(crate) fn fname_from_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into()
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
impl MyJoin for BTreeSet<String> {
    fn join(&self, sep: &str) -> String {
        self.iter().map(|item| item.as_str()).collect::<Vec<&str>>().join(sep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashset_join() {
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