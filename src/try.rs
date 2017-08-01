use error::*;

pub trait Try<T> {
    fn ok_or_then<F: FnOnce() -> CDResult<T>>(self, cb: F) -> CDResult<T>;
}

impl<T> Try<T> for Option<T> {
    fn ok_or_then<F: FnOnce() -> CDResult<T>>(self, cb: F) -> CDResult<T> {
        if let Some(s) = self {
            Ok(s)
        } else {
            cb()
        }
    }
}
