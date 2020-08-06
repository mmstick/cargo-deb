/// This module is a partial implementation of the Debian DebHelper core library
/// aka dh_lib. Specifically this implementation is based on the Ubuntu version
/// labelled 12.10ubuntu1 which is included in Ubuntu 20.04 LTS. I believe 12 is
/// a reference to Debian 12 "Bookworm", i.e. Ubuntu uses future Debian sources
/// and is also referred to as compat level 12 by debhelper documentation. Only
/// functionality that was needed to properly script installation of systemd
/// units, i.e. that used by the debhelper dh_instalsystemd command or rather
/// our dh_installsystemd.rs implementation of it, is included here.
/// 
/// # See also
/// 
/// Ubuntu 20.04 dh_lib sources:
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1
/// 
/// Ubuntu 20.04 dh_installsystemd man page (online HTML version):
/// http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installdeb.1.html

use rust_embed::RustEmbed;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{CDResult, listener::Listener};
use crate::error::*;

/// DebHelper autoscripts are embedded in the Rust library binary.
/// The autoscripts were taken from:
///   https://git.launchpad.net/ubuntu/+source/debhelper/tree/autoscripts?h=applied/12.10ubuntu1
/// To understand which scripts are invoked when, consult:
///   https://www.debian.org/doc/debian-policy/ap-flowcharts.htm
#[derive(RustEmbed)]
#[folder = "autoscripts/"]
struct Autoscripts;

pub(crate) type ScriptFragments = HashMap<String, Vec<u8>>;

/// Find a file in the given directory that best matches the given package,
/// filename and (optional) unit name. Enables callers to use the most specific
/// match while also falling back to a less specific match (e.g. a file to be
/// used as a default) when more specific matches are not available.
/// 
/// Returns one of the following, in order of most preferred first:
/// 
///   - Some("<dir>/<package>.<unit_name>.<filename>")
///   - Some("<dir>/<package>.<filename>")
///   - Some("<dir>/<unit_name>.<filename>")
///   - Some("<dir>/<filename>")
///   - None
/// 
/// <filename> is either a systemd unit type such as `service` or `socket`, or a
/// maintainer script name such as `postinst`.
///
/// # Known limitations
/// 
/// The pkgfile() subroutine in the actual dh_installsystemd code is capable of
/// matching architecture and O/S specific unit files, but this implementation
/// does not support architecture or O/S specific unit files.
/// 
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n957
pub(crate) fn pkgfile(dir: &Path, package: &str, filename: &str, unit_name: Option<&str>)
     -> Option<PathBuf>
{
    // From man 1 dh_installsystemd on Ubuntu 20.04 LTS. See:
    //   http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installsystemd.1.html
    // --name=name
    //     ...
    //     It changes the name that dh_installsystemd uses when it looks for
    //     maintainer provided systemd unit files as listed in the "FILES"
    //     section.  As an example, dh_installsystemd --name foo will look for
    //     debian/package.foo.service instead of debian/package.service).  These
    //     unit files are installed as name.unit-extension (in the example, it
    //     would be installed as foo.service).
    //     ...
    let named_filename = if let Some(str) = unit_name {
        format!("{}.{}", str, filename)
    } else {
        filename.to_owned()
    };

    let mut paths_to_try = Vec::new();
    paths_to_try.push(dir.join(format!("{}.{}", package, named_filename)));
    paths_to_try.push(dir.join(format!("{}.{}", package, filename)));
    paths_to_try.push(dir.join(named_filename.clone()));
    paths_to_try.push(dir.join(filename.clone()));

    fn is_file(path: &PathBuf) -> bool {
        cfg_if! {
            if #[cfg(test)] {
                // Avoid accessing the filesystem when testing so that we don't
                // have to create real directories and files in unique locations
                // for each test run in parallel, uggh.
                path.mock_is_file()
            } else {
                path.is_file()
            }
        }
    }

    for path_to_try in paths_to_try {
        if is_file(&path_to_try) {
            return Some(path_to_try);
        }
    }

    None
}

/// Get the bytes for the specified filename whose contents were embedded in our
/// binary by the rust-embed crate. See #[derive(RustEmbed)] above.
fn get_embedded_autoscript(snippet_filename: &str) -> String {
    // load
    let snippet = Autoscripts::get(snippet_filename)
        .expect(&format!("Unknown autoscript '{}'", snippet_filename));

    // convert to string
    let mut snippet = String::from(std::str::from_utf8(snippet.as_ref()).unwrap());

    // normalize
    if !snippet.ends_with('\n') {
        snippet.push('\n');
    }

    // return
    snippet
}

/// Build up one or more shell script fragments for a given maintainer script
/// for a debian package in preparation for writing them into or as complete
/// maintainer scripts in `apply()`, pulling fragments from a "library" of
/// so-called "autoscripts".
/// 
/// Takes a map of values to search and replace in the selected "autoscript"
/// fragment such as a systemd unit name placeholder and value.
/// 
/// # Cargo Deb specific behaviour
/// 
/// The autoscripts are sourced from within the binary via the rust_embed crate.
/// 
/// Results are stored as updated or new entries in the `ScriptFragments` map,
/// rather than being written to temporary files on disk.
/// 
/// # Known limitations
/// 
/// Arbitrary sed command based file editing is not supported.
/// 
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1135
pub(crate) fn autoscript(
    scripts: &mut ScriptFragments,
    package: &str,
    script: &str,
    snippet_filename: &str,
    replacements: &HashMap<&str, String>,
    listener: &mut dyn Listener) -> CDResult<()>
{
    let bin_name = std::env::current_exe().unwrap();
    let bin_name = bin_name.file_name().unwrap();
    let bin_name = bin_name.to_str().unwrap();
    let outfile = format!("{}.{}.debhelper", package, script);

    listener.info(format!("Maintainer script {} will be augmented with autoscript {}", &script, snippet_filename));

    if scripts.contains_key(&outfile) && (script == "postrm" || script == "prerm") {
        if !replacements.is_empty() {
            let existing_text = std::str::from_utf8(scripts.get(&outfile).unwrap())?;

            // prepend new text to existing script fragment
            let mut new_text = String::new();
            new_text.push_str(&format!("# Automatically added by {}\n", bin_name));
            new_text.push_str(&autoscript_sed(snippet_filename, replacements));
            new_text.push_str("# End automatically added section\n");
            new_text.push_str(existing_text);
            scripts.insert(outfile, new_text.into());
        } else {
            // We don't support sed commands yet.
            unimplemented!();
        }
    } else if !replacements.is_empty() {
        // append to existing script fragment (if any)
        let mut new_text = String::from(std::str::from_utf8(scripts.get(&outfile).unwrap_or(&Vec::new()))?);
        new_text.push_str(&format!("# Automatically added by {}\n", bin_name));
        new_text.push_str(&autoscript_sed(snippet_filename, replacements));
        new_text.push_str("# End automatically added section\n");
        scripts.insert(outfile, new_text.into());
    } else {
        // We don't support sed commands yet.
        unimplemented!();
    }

    Ok(())
}

/// Search and replace a collection of key => value pairs in the given file and
/// return the resulting text as a String.
/// 
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1203
fn autoscript_sed(snippet_filename: &str, replacements: &HashMap<&str, String>) -> String {
    let mut snippet = get_embedded_autoscript(snippet_filename);

    for (from, to) in replacements {
        snippet = snippet.replace(&format!("#{}#", from), to);
    }

    snippet
}

/// Copy the merged autoscript fragments to the final maintainer script, either
/// at the point where the user placed a #DEBHELPER# token to indicate where
/// they should be inserted, or by adding a shebang header to make the fragments
/// into a complete shell script.
///
/// # Cargo Deb specific behaviour
/// 
/// Results are stored as updated or new entries in the `ScriptFragments` map,
/// rather than being written to temporary files on disk.
/// 
/// # Known limitations
/// 
/// We only replace #DEBHELPER#. Is that enough? See:
///   https://www.man7.org/linux/man-pages/man1/dh_installdeb.1.html#SUBSTITUTION_IN_MAINTAINER_SCRIPTS
///
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n2161
fn debhelper_script_subst(user_scripts_dir: &Path, scripts: &mut ScriptFragments, package: &str, script: &str, unit_name: Option<&str>,
    listener: &mut dyn Listener) -> CDResult<()>
{
    let user_file = pkgfile(user_scripts_dir, package, script, unit_name);
    let generated_file_name = format!("{}.{}.debhelper", package, script);

    if let Some(user_file_path) = user_file {
        listener.info(format!("Augmenting maintainer script {}", user_file_path.display()));

        // merge the generated scripts if they exist into the user script
        // if no generated script exists, we still need to remove #DEBHELPER# if
        // present otherwise the script will be syntactically invalid
        let generated_text = match scripts.get(&generated_file_name) {
            Some(contents) => String::from_utf8(contents.clone())?,
            None           => String::from("")
        };
        let user_text = std::fs::read_to_string(user_file_path.as_path())?;
        let new_text = user_text.replace("#DEBHELPER#", &generated_text);
        if new_text == user_text {
            return Err(CargoDebError::DebHelperReplaceFailed(user_file_path));
        }
        scripts.insert(script.into(), new_text.into());
    } else if let Some(generated_bytes) = scripts.get(&generated_file_name) {
        listener.info(format!("Generating maintainer script {}", script));

        // give it a shebang header and rename it
        let mut new_text = String::new();
        new_text.push_str("#!/bin/sh\n");
        new_text.push_str("set -e\n");
        new_text.push_str(std::str::from_utf8(generated_bytes)?);

        scripts.insert(script.into(), new_text.into());
    }

    Ok(())
}

/// Generate final maintainer scripts by merging the autoscripts that have been
/// collected in the `ScriptFragments` map  with the maintainer scripts
/// on disk supplied by the user.
/// 
/// See: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installdeb?h=applied/12.10ubuntu1#n300
pub(crate) fn apply(user_scripts_dir: &Path, scripts: &mut ScriptFragments, package: &str, unit_name: Option<&str>,
    listener: &mut dyn Listener) -> CDResult<()>
{
    for script in &["postinst", "preinst", "prerm", "postrm"] {
        // note: we don't support custom defines thus we don't have the final
        // 'package_subst' argument to debhelper_script_subst().
        debhelper_script_subst(user_scripts_dir, scripts, package, script, unit_name, listener)?;
    }

    Ok(())
}

cfg_if! {
    if #[cfg(test)] {
        // ---------------------------------------------------------------------
        // Begin: testable extension to PathBuf
        // ---------------------------------------------------------------------
        // The pkgfile() function accesses the filesystem directly via its use
        // the Path(Buf)::is_file() method which checks for the existence of a
        // file in the real filesystem.
        //
        // To test this without having to create real files and directories we
        // extend the PathBuf type via a trait with a mock_is_file() method
        // which, in test builds, is used by pkgfile() instead of the real
        // PathBuf::is_file() method.
        //
        // The mock_is_file() method looks up the current path in a vector which
        // represents a set of paths in a virtual filesystem. I don't know of a
        // way to make additional state available to the trait, e.g. AFAIK it
        // cannot yet have its own fields, and so it can only check global
        // state. However, accessing global state in a multithreaded test run is
        // unsafe, plus we want each test to define its own virtual filesystem
        // to test against, not a single global virtual filesystem shared by all
        // tests.
        //
        // To implement this test specific virtual filesystem I use a vector,
        // protected by a thread local vector such that each test (thread) gets
        // its own copy of the vector. To be able to mutate the vector I protect
        // it with a Mutex. To make this setup easier to work with I define a
        // couple of helpher functions:
        //
        //   - add_test_fs_paths() - adds paths to the current tests virtual fs
        //   - with_test_fs() - passes the current tests virtual fs vector to
        //                      a user defined callback function.
        use std::sync::Mutex;

        thread_local!(
            static MOCK_FS: Mutex<Vec<&'static str>> = Mutex::new(vec![])
        );

        fn add_test_fs_paths(paths: &Vec<&'static str>) {
            MOCK_FS.with(|fs| fs.lock().unwrap().extend(paths));
        }

        fn with_test_fs<F, R>(callback: F) -> R
        where
            F: Fn(&Vec<&'static str>) -> R
        {
            MOCK_FS.with(|fs| callback(&fs.lock().unwrap()))
        }

        pub(crate) trait TestablePath {
            fn mock_is_file(&self) -> bool;
        }

        impl TestablePath for PathBuf {
            fn mock_is_file(&self) -> bool {
                with_test_fs(|fs| {
                    fs.contains(&self.to_str().unwrap())
                })
            }
        }
        // ---------------------------------------------------------------------
        // End: testable extension to PathBuf
        // ---------------------------------------------------------------------

        mod tests {
            use super::*;
            use rstest::*;

            // helper conversion
            // create a new type to work around error "only traits defined in
            // the current crate can be implemented for arbitrary types"
            #[derive(Debug)]
            struct LocalOptionPathBuf(Option<PathBuf>);
            // Implement <&str> == <LocalOptionPathBuf> comparisons
            impl PartialEq<LocalOptionPathBuf> for &str {
                fn eq(&self, other: &LocalOptionPathBuf) -> bool {
                    Some(Path::new(self).to_path_buf()) == other.0
                }
            }
            // Implement <LocalOptionPathBuf> == <&str> comparisons
            impl PartialEq<&str> for LocalOptionPathBuf {
                fn eq(&self, other: &&str) -> bool {
                    self.0 == Some(Path::new(*other).to_path_buf())
                }
            }

            #[test]
            fn pkgfile_finds_most_specific_match_with_pkg_unit_file() {
                add_test_fs_paths(&vec![
                    "/parent/dir/postinst",
                    "/parent/dir/myunit.postinst",
                    "/parent/dir/mypkg.postinst",
                    "/parent/dir/mypkg.myunit.postinst",
                    "/parent/dir/nested/mypkg.myunit.postinst",
                    "/parent/mypkg.myunit.postinst",
                ]);

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", Some("myunit"));
                assert_eq!("/parent/dir/mypkg.myunit.postinst", LocalOptionPathBuf(r));

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", None);
                assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));
            }

            #[test]
            fn pkgfile_finds_most_specific_match_without_unit_file() {
                add_test_fs_paths(&vec![
                    "/parent/dir/postinst",
                    "/parent/dir/myunit.postinst",
                    "/parent/dir/mypkg.postinst",
                    "/parent/dir/nested/mypkg.myunit.postinst",
                    "/parent/mypkg.myunit.postinst",
                ]);

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", Some("myunit"));
                assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", None);
                assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));
            }

            #[test]
            fn pkgfile_finds_most_specific_match_without_pkg_file() {
                add_test_fs_paths(&vec![
                    "/parent/dir/postinst",
                    "/parent/dir/myunit.postinst",
                ]);

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", Some("myunit"));
                assert_eq!("/parent/dir/myunit.postinst", LocalOptionPathBuf(r));

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", None);
                assert_eq!("/parent/dir/postinst", LocalOptionPathBuf(r));
            }
  
            #[test]
            fn pkgfile_finds_a_fallback_match() {
                add_test_fs_paths(&vec![
                    "/parent/dir/postinst",
                    "/parent/dir/myunit.postinst",
                    "/parent/dir/mypkg.postinst",
                    "/parent/dir/mypkg.myunit.postinst",
                    "/parent/dir/nested/mypkg.myunit.postinst",
                    "/parent/mypkg.myunit.postinst",
                ]);

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "postinst", Some("wrongunit"));
                assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));

                let r = pkgfile(Path::new("/parent/dir/"), "wrongpkg", "postinst", None);
                assert_eq!("/parent/dir/postinst", LocalOptionPathBuf(r));
            }

            #[test]
            fn pkgfile_fails_to_find_a_match() {
                add_test_fs_paths(&vec![
                    "/parent/dir/postinst",
                    "/parent/dir/myunit.postinst",
                    "/parent/dir/mypkg.postinst",
                    "/parent/dir/mypkg.myunit.postinst",
                    "/parent/dir/nested/mypkg.myunit.postinst",
                    "/parent/mypkg.myunit.postinst",
                ]);

                let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "wrongfile", None);
                assert_eq!(None, r);

                let r = pkgfile(Path::new("/wrong/dir/"), "mypkg", "postinst", None);
                assert_eq!(None, r);
            }

            fn autoscript_test_wrapper(pkg: &str, script: &str, snippet: &str, unit: &str, scripts: Option<ScriptFragments>)
                -> ScriptFragments
            {
                let mut mock_listener = crate::listener::MockListener::new();
                mock_listener.expect_info().times(1).return_const(());
                let mut scripts = scripts.unwrap_or(ScriptFragments::new());
                let replacements = map!{ "UNITFILES" => unit.to_owned() };
                autoscript(&mut scripts, pkg, script, snippet, &replacements, &mut mock_listener).unwrap();
                return scripts;
            }

            #[test]
            #[should_panic(expected = "Unknown autoscript 'idontexist'")]
            fn autoscript_panics_with_unknown_autoscript() {
                autoscript_test_wrapper("mypkg", "somescript", "idontexist", "dummyunit", None);
            }

            #[test]
            #[should_panic(expected = "not implemented")]
            fn autoscript_panics_in_sed_mode() {
                let mut mock_listener = crate::listener::MockListener::new();
                mock_listener.expect_info().times(1).return_const(());
                let mut scripts = ScriptFragments::new();
                autoscript(&mut scripts, "mypkg", "somescript", "idontexist", &HashMap::new(), &mut mock_listener).unwrap();
            }

            #[test]
            fn autoscript_check_embedded_files() {
                let mut actual_scripts: Vec<std::borrow::Cow<'static, str>> = Autoscripts::iter().collect();
                actual_scripts.sort();

                let expected_scripts = vec![
                    "postinst-init-tmpfiles",
                    "postinst-systemd-dont-enable",
                    "postinst-systemd-enable",
                    "postinst-systemd-restart",
                    "postinst-systemd-restartnostart",
                    "postinst-systemd-start",
                    "postrm-systemd",
                    "postrm-systemd-reload-only",
                    "prerm-systemd",
                    "prerm-systemd-restart",
                ];

                assert_eq!(expected_scripts, actual_scripts);
            }

            #[test]
            fn autoscript_sanity_check_with_embedded_snippets() {
                for snippet_filename in Autoscripts::iter() {
                    autoscript_test_wrapper("mypkg", "somescript", &snippet_filename, "dummyunit", None);
                }
            }

            #[rstest(maintainer_script, prepend,
                case::prerm("prerm", true),
                case::preinst("preinst", false),
                case::postinst("postinst", false),
                case::postrm("postrm", true),
            )]
            fn autoscript_detailed_check(maintainer_script: &str, prepend: bool) {
                let autoscript_name = "postrm-systemd";

                // Populate an autoscript template and add the result to a
                // collection of scripts and return it to us.
                let scripts = autoscript_test_wrapper("mypkg", maintainer_script, &autoscript_name, "dummyunit", None);

                // Expect autoscript() to have created one temporary script
                // fragment called <package>.<script>.debhelper.
                assert_eq!(1, scripts.len());

                let expected_created_name = &format!("mypkg.{}.debhelper", maintainer_script);
                let (created_name, created_bytes) = scripts.iter().next().unwrap();

                // Verify the created script filename key
                assert_eq!(expected_created_name, created_name);

                // Verify the created script contents. It should have two lines
                // more than the autoscript fragment it was based on, like so:
                //   # Automatically added by ...
                //   <autoscript fragment lines with placeholders replaced>
                //   # End automatically added section
                let autoscript_text = get_embedded_autoscript(autoscript_name);
                let autoscript_line_count = autoscript_text.lines().count();
                let created_text = std::str::from_utf8(created_bytes).unwrap();
                let created_line_count = created_text.lines().count();
                assert_eq!(autoscript_line_count + 2, created_line_count);

                // Verify the content of the added comment lines
                let mut lines = created_text.lines();
                assert!(lines.nth(0).unwrap().starts_with("# Automatically added by"));
                assert_eq!(lines.nth_back(0).unwrap(), "# End automatically added section");

                // Check that the autoscript fragment lines were properly copied
                // into the created script complete with expected substitutions
                let expected_autoscript_text1 = autoscript_text.replace("#UNITFILES#", "dummyunit");
                let expected_autoscript_text1 = expected_autoscript_text1.trim_end();
                let start1 = 1; let end1 = start1 + autoscript_line_count;
                let created_autoscript_text1 = created_text.lines().collect::<Vec<&str>>()[start1..end1].join("\n");
                assert_ne!(expected_autoscript_text1, autoscript_text);
                assert_eq!(expected_autoscript_text1, created_autoscript_text1);

                // Process the same autoscript again but use a different unit
                // name so that we can see if the autoscript template was again
                // populated but this time with the different value, and pass in
                // the existing set of created scripts to check how it gets
                // modified.
                let scripts = autoscript_test_wrapper("mypkg", maintainer_script, &autoscript_name, "otherunit", Some(scripts));

                // The number and name of the output scripts should remain the same
                assert_eq!(1, scripts.len());
                let (created_name, created_bytes) = scripts.iter().next().unwrap();
                assert_eq!(expected_created_name, created_name);

                // The line structure should now contain two injected blocks
                let created_text = std::str::from_utf8(created_bytes).unwrap();
                let created_line_count = created_text.lines().count();
                assert_eq!((autoscript_line_count + 2) * 2, created_line_count);

                let mut lines = created_text.lines();
                assert!(lines.nth(0).unwrap().starts_with("# Automatically added by"));
                assert_eq!(lines.nth_back(0).unwrap(), "# End automatically added section");

                // The content should be different
                let expected_autoscript_text2 = autoscript_text.replace("#UNITFILES#", "otherunit");
                let expected_autoscript_text2 = expected_autoscript_text2.trim_end();
                let start2 = end1 + 2; let end2 = start2 + autoscript_line_count;
                let created_autoscript_text1 = created_text.lines().collect::<Vec<&str>>()[start1..end1].join("\n");
                let created_autoscript_text2 = created_text.lines().collect::<Vec<&str>>()[start2..end2].join("\n");
                assert_ne!(expected_autoscript_text1, autoscript_text);
                assert_ne!(expected_autoscript_text2, autoscript_text);

                if prepend {
                    assert_eq!(expected_autoscript_text1, created_autoscript_text2);
                    assert_eq!(expected_autoscript_text2, created_autoscript_text1);
                } else {
                    assert_eq!(expected_autoscript_text1, created_autoscript_text1);
                    assert_eq!(expected_autoscript_text2, created_autoscript_text2);
                }
            }
       }
    }
}