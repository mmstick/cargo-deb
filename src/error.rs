use std::io;
use std::num;
use std::time;
use compress::{CompressErr, Archive};
use toml;

quick_error! {
    #[derive(Debug)]
    pub enum CargoDebError {
        Io(err: io::Error) {
            from()
            description(err.description())
            display("I/O error: {}", err)
            cause(err)
        }
        IoFile(err: io::Error, file: String) {
            description(err.description())
            display("I/O error: {}", file)
            cause(err)
        }
        CommandFailed(err: io::Error, cmd: &'static str) {
            description(err.description())
            display("Command {} failed to launch", cmd)
            cause(err)
        }
        CommandError(msg: &'static str, arg: String, reason: Vec<u8>) {
            description(msg)
            display("{} ({}): {}", msg, arg, String::from_utf8_lossy(&reason))
        }
        Str(msg: &'static str) {
            from()
            description(msg)
        }
        NumParse(msg: &'static str, err: num::ParseIntError) {
            description(msg)
            cause(err)
        }
        InstallFailed {
            description("dpkg install failed")
        }
        BuildFailed {
            description("build failed")
        }
        StripFailed(name: String) {
            description("strip failed")
            display("strip failed: {}", name)
        }
        SystemTime(err: time::SystemTimeError) {
            from()
            description("unable to get system time")
            cause(err)
        }
        Parse(err: toml::de::Error) {
            from()
            description(err.description())
            display("TOML error: {}", err)
            cause(err)
        }
        PackageNotFound(path: String, reason: Vec<u8>) {
            description("unable to find package for the library")
            display("path '{}' does not belong to a package: {}", path, String::from_utf8_lossy(&reason))
        }
        NotInstalled(package: String) {
            description("required dependencies are not installed")
            display("dependency package '{}' is not installed", package)
        }
        GetVersionError(package: String) {
            description("unable to get version of a package")
            display("unable to get version of '{}'", package)
        }
        Compress(err: CompressErr) {
            from()
            description(match *err {
                CompressErr::Compression(_) => "error with zopfli compression",
                // The application was unable to create the `target/debian` directory.
                CompressErr::UnableToCreatePath(_) => "unable to create 'target/debian'",
                // The application was unable to write the archive to disk.
                CompressErr::Write(Archive::Control, _) => "unable to write to 'target/debian/control.tar.gz'",
                CompressErr::Write(Archive::Data, _) => "unable to write to 'target/debian/data.tar.xz'",
            })
            display("{}", match *err {
                CompressErr::Compression(ref reason) => reason.clone(),
                CompressErr::UnableToCreatePath(ref reason) => reason.to_string(),
                CompressErr::Write(_, ref reason) => reason.to_string(),
            })
        }
    }
}

pub type CDResult<T> = Result<T, CargoDebError>;
