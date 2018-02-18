pub trait Listener {
    fn warning(&mut self, s: String);
    fn info(&mut self, s: String);
}

pub struct NoOpListener;
impl Listener for NoOpListener {
    fn info(&mut self, _s: String) {}
    fn warning(&mut self, _s: String) {}
}

pub struct StdErrListener {
    pub verbose: bool,
}
impl Listener for StdErrListener {
    fn warning(&mut self, s: String) {
        eprintln!("warning: {}", s);
    }
    fn info(&mut self, s: String) {
        if self.verbose {
            eprintln!("info: {}", s);
        }
    }
}
