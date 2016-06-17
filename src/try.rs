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
        self.unwrap_or_else(|_| {
            let mut stderr = io::stderr();
            stderr.write(error.as_bytes()).unwrap();
            stderr.flush().unwrap();
            exit(1);
        })
    }
}

impl<T> Try for Option<T> {
    type Succ = T;

    fn try(self, error: &str) -> T {
        self.unwrap_or_else(|| {
            let mut stderr = io::stderr();
            stderr.write(error.as_bytes()).unwrap();
            stderr.flush().unwrap();
            exit(1);
        })
    }
}

pub fn failed<T: AsRef<str>>(input: T) {
    let input = input.as_ref();
    let mut stderr = io::stderr();
    stderr.write(input.as_bytes()).unwrap();
    stderr.flush().unwrap();
    exit(1);
}
