pub struct Fmter<'f> {
    pub leading_spaces: String, // ugly, better ways to do it..
    pub w: &'f mut dyn std::fmt::Write,
}
impl<'w> Fmter<'w> {
    pub fn new(w: &'w mut impl std::fmt::Write) -> Self {
        Self {
            leading_spaces: String::new(),
            w,
        }
    }
    pub fn indent(&mut self) {
        const INDENT: &'static str = "    ";
        self.leading_spaces.push_str(INDENT);
    }
    pub fn unindent(&mut self) {
        self.leading_spaces.truncate(self.leading_spaces.len() - 4)
    }
}

macro_rules! indented {
    ($fmter:expr $(,)?) => {
        write!($fmter.w, "\n").unwrap()
    };
    ($fmter:expr, $($arg:tt)*) => {
        write!($fmter.w, "{}",$fmter.leading_spaces).and_then(|_| writeln!($fmter.w, $($arg)*)).unwrap()
    };
}
