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
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use crate::listener::Listener;

/// DebHelper autoscripts are embedded in the Rust library binary. For more
/// information about the source of the scripts see `autoscripts/README.md`.
#[derive(RustEmbed)]
#[folder = "autoscripts/"]
struct Autoscripts;

/// Find a file in the given directory that best match the given package,
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
pub(crate) fn pkgfile(dir: &PathBuf, package: &str, filename: &str, unit_name: Option<&str>)
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

    for path_to_try in paths_to_try {
        if path_to_try.is_file() {
            return Some(path_to_try);
        }
    }

    None
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
/// # Known limitations
/// 
/// Arbitrary sed command based file editing is not supported.
/// 
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1135
pub(crate) fn autoscript(
    tmp_dir: &Path,
    package: &str,
    script: &str,
    snippet_filename: &str,
    replacements: &HashMap<&str, String>,
    listener: &mut dyn Listener)
{
    let bin_name = std::env::current_exe().unwrap();
    let bin_name = bin_name.file_name().unwrap();
    let bin_name = bin_name.to_str().unwrap();
    let outfile = tmp_dir.join(format!("{}.{}.debhelper", package, script));

    listener.info(format!("Maintainer script {} will be augmented with autoscript {}", &script, snippet_filename));

    if outfile.exists() && (script == "postrm" || script == "prerm") {
        if !replacements.is_empty() {
            // prepend new text to existing file
            let mut new_text = String::new();
            new_text.push_str(&format!("# Automatically added by {}\n", bin_name));
            new_text.push_str(&autoscript_sed(snippet_filename, replacements));
            new_text.push_str("# End automatically added section\n");
            new_text.push_str(&std::fs::read_to_string(&outfile).unwrap());
            let mut file = File::create(&outfile).unwrap(); // deletes the original, if any
            file.write_all(&new_text.into_bytes()).unwrap();
        } else {
            // We don't support sed commands yet.
            unimplemented!();
        }
    } else if !replacements.is_empty() {
        let mut new_text = String::new();
        new_text.push_str(&format!("# Automatically added by {:?}\n", bin_name));
        new_text.push_str(&autoscript_sed(snippet_filename, replacements));
        new_text.push_str("# End automatically added section\n");
        let mut file = if !outfile.exists() {
            File::create(&outfile).unwrap()
        } else {
            OpenOptions::new().append(true).open(&outfile).unwrap()
        };
        file.write_all(&new_text.into_bytes()).unwrap();
    } else {
        // We don't support sed commands yet.
        unimplemented!();
    }
}

/// Search and replace a collection of key => value pairs in the given file and
/// return the resulting text as a String.
/// 
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1203
fn autoscript_sed(
    snippet_filename: &str,
    replacements: &HashMap<&str, String>)
        -> String
{
    let snippet = Autoscripts::get(snippet_filename).unwrap();
    let mut snippet = String::from(std::str::from_utf8(snippet.as_ref()).unwrap());
    for (from, to) in replacements {
        snippet = snippet.replace(&format!("#{}#", from), to);
    }
    String::from(snippet)
}

/// Copy the merged autoscript fragments to the final maintainer script, either
/// at the point where the user placed a #DEBHELPER# token to indicate where
/// they should be inserted, or by adding a shebang header to make the fragments
/// into a complete shell script.
/// 
/// # Known limitations
/// 
/// We only replace #DEBHELPER#. Is that enough? See:
///   https://www.man7.org/linux/man-pages/man1/dh_installdeb.1.html#SUBSTITUTION_IN_MAINTAINER_SCRIPTS
///
/// # References
///
/// https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n2161
fn debhelper_script_subst(tmp_dir: &PathBuf, package: &str, script: &str, unit_name: Option<&str>,
    listener: &mut dyn Listener)
{
    let user_file = pkgfile(tmp_dir, package, script, unit_name);
    let generated_file = tmp_dir.join(format!("{}.{}.debhelper", package, script));
    let out_file = tmp_dir.join(script);

    if user_file.is_some() {
        listener.info(format!("Augmenting maintainer script {}", script));

        // merge the generated scripts if they exist into the user script
        // if no generated script exists, we still need to remove #DEBHELPER# if
        // present otherwise the script will be syntactically invalid
        let generated_text = if generated_file.exists() {
            std::fs::read_to_string(generated_file).unwrap()
        } else {
            String::from("")
        };
        let user_text = std::fs::read_to_string(user_file.clone().unwrap()).unwrap();
        let new_text = user_text.replace("#DEBHELPER#", &generated_text);
        if new_text == user_text {
            panic!("#DEBHELPER# token not found in maintainer script {:?}",
                user_file.unwrap().strip_prefix(tmp_dir).unwrap());
        }
        let mut file = File::create(out_file).unwrap(); // deletes the original, if any
        file.write_all(&new_text.into_bytes()).unwrap();
    } else if generated_file.exists() {
        listener.info(format!("Generating maintainer script {}", script));
        // give it a shebang header and rename it
        let mut new_text = String::new();
        new_text.push_str("#!/bin/sh\n");
        new_text.push_str("set -e\n");
        new_text.push_str(&std::fs::read_to_string(&generated_file).unwrap());
        let mut file = File::create(out_file).unwrap(); // deletes the original, if any
        file.write_all(&new_text.into_bytes()).unwrap();
    }
}

/// Generate final maintainer scripts by merging the autoscripts that have been
/// collected in the temp directory with the originals supplied by the user.
/// 
/// See: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installdeb?h=applied/12.10ubuntu1#n300
pub(crate) fn apply(tmp_dir: &PathBuf, package: &str, unit_name: Option<&str>,
    listener: &mut dyn Listener)
{
    for script in &["postinst", "preinst", "prerm", "postrm"] {
        // note: we don't support custom defines thus we don't have the a last
        // 'package_subst' argument to debhelper_script_subst().
        debhelper_script_subst(tmp_dir, package, script, unit_name, listener);
    }
}