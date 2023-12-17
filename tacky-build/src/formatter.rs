use std::fmt::Write;

pub struct Fmter<'f, W> {
    leading_spaces: String, //ugly, better ways to do it..
    w: &'f mut W,
}
struct Fnwriter;

impl Fnwriter {
    fn signature() {}
    fn body() {}
}
impl<W> Fmter<'_, W> {
    pub fn indent(&mut self) {
        const INDENT: &'static str = "    ";
        self.leading_spaces.push_str(INDENT);
    }
    pub fn unindent(&mut self) {
        self.leading_spaces.truncate(self.leading_spaces.len() - 4)
    }
    pub fn level(&self) -> usize {
        self.leading_spaces.len()
    }
}

impl<'f, W: std::fmt::Write> std::fmt::Write for Fmter<'f, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.w.write_str(&self.leading_spaces)?;
        self.w.write_str(s)
    }
}

#[test]
fn testme() {
    let mut buf = String::new();
    let mut fmter = Fmter {
        leading_spaces: String::new(),
        w: &mut buf,
    };
    writeln!(&mut fmter, "pub fn testme() -> bool {{");
    fmter.indent();
    writeln!(&mut fmter, "body");
    fmter.unindent();
    writeln!(&mut fmter, "}}");
    println!("{buf}")
}
