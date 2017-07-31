use std::io::{self, Write};
use std::process::exit;
use std::error::Error;

pub trait Try {
    type Succ;

    fn try(self, error: &str) -> Self::Succ;
}

impl<T, U: Error> Try for Result<T, U> {
    type Succ = T;

    fn try(self, error: &str) -> T {
        self.unwrap_or_else(|reason| {
            let _ = writeln!(&mut io::stderr(), "cargo-deb: {}: {}", error, reason.to_string());
            exit(1);
        })
    }
}

impl<T> Try for Option<T> {
    type Succ = T;

    fn try(self, error: &str) -> T {
        self.unwrap_or_else(|| {
            let _ = writeln!(&mut io::stderr(), "cargo-deb: {}", error);
            exit(1);
        })
    }
}

pub fn failed<T: AsRef<str>>(input: T) -> ! {
    let input = input.as_ref();
    let _ = writeln!(&mut io::stderr(), "cargo-deb: {}", input);
    exit(1);
}
