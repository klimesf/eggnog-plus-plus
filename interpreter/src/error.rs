//! Source positions and error reporting, shared by every stage.

use std::fmt;

/// A range of bytes in the source file: `start..end`.
///
/// Every token and AST node carries a span so that errors (and later,
/// runtime traps) can point at the exact place in the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    /// Useful in the parser: the span of `a + b` is `a.span.to(b.span)`.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// Which stage produced the error. Only used to label the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Lex,
    Parse,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub stage: Stage,
    pub message: String,
    pub span: Span,
}

impl Error {
    pub fn lex(message: impl Into<String>, span: Span) -> Self {
        Error { stage: Stage::Lex, message: message.into(), span }
    }

    pub fn parse(message: impl Into<String>, span: Span) -> Self {
        Error { stage: Stage::Parse, message: message.into(), span }
    }

    pub fn runtime(message: impl Into<String>, span: Span) -> Self {
        Error { stage: Stage::Runtime, message: message.into(), span }
    }

    /// Formats the error with the offending source line and a caret underline:
    ///
    /// ```text
    /// parse error: expected ':' after condition
    ///  --> day10.egg:3:14
    ///   |
    /// 3 | while x > 0
    ///   |             ^
    /// ```
    pub fn render(&self, file_name: &str, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = source[start..].find('\n').map_or(source.len(), |i| start + i);
        let line_no = source[..line_start].matches('\n').count() + 1;
        let col = start - line_start + 1;
        let line = &source[line_start..line_end];
        let width = (self.span.end.min(line_end).saturating_sub(start)).max(1);
        let gutter = " ".repeat(line_no.to_string().len());

        format!(
            "{stage} error: {msg}\n{gutter}--> {file_name}:{line_no}:{col}\n{gutter} |\n{line_no} | {line}\n{gutter} | {pad}{carets}\n",
            stage = self.stage,
            msg = self.message,
            pad = " ".repeat(col - 1),
            carets = "^".repeat(width),
        )
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Stage::Lex => "lex",
            Stage::Parse => "parse",
            Stage::Runtime => "runtime",
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} error at {}..{}: {}", self.stage, self.span.start, self.span.end, self.message)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_points_at_the_span() {
        let src = "let a = 1\nlet b = a +\n";
        let err = Error::parse("expected an expression", Span::new(20, 21));
        let out = err.render("t.egg", src);
        assert_eq!(
            out,
            "parse error: expected an expression\n --> t.egg:2:11\n  |\n2 | let b = a +\n  |           ^\n"
        );
    }
}
