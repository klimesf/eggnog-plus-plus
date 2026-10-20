use std::fmt;

/// A byte range `start..end` in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Lex,
    Parse,
    Runtime,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub stage: Stage,
    pub message: String,
    pub span: Span,
}

pub type Result<T> = std::result::Result<T, Error>;

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

    /// Formats the error with the source line and a `^^^` underline.
    /// `first_line` is the line number of the first line of `source` in the
    /// file (1 for a whole file; larger for a code block inside Markdown).
    pub fn render(&self, file_name: &str, source: &str, first_line: usize) -> String {
        let start = self.span.start.min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = source[start..].find('\n').map_or(source.len(), |i| start + i);
        let line_no = source[..line_start].matches('\n').count() + first_line;
        let col = start - line_start + 1;
        let line = &source[line_start..line_end];
        let width = self.span.end.min(line_end).saturating_sub(start).max(1);
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
