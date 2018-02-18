pub trait Listener {
    fn warning(&mut self, s: String);
}

pub struct NoOpListener;
impl Listener for NoOpListener {
    fn warning(&mut self, _s: String) {}
}

pub struct StdErrListener;
impl Listener for StdErrListener {
    fn warning(&mut self, s: String) {
        eprintln!("warning: {}", s);
    }
}
