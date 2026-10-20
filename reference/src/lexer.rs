//! Source text → tokens, including INDENT/DEDENT, line continuation and
//! triple-quoted strings.

use std::rc::Rc;

use crate::error::{Error, Result, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Int(i64),
    Str(Rc<str>),
    Char(u8),
    Ident(Rc<str>),

    Let,
    Fun,
    Memo,
    Inout,
    If,
    Then,
    Else,
    While,
    For,
    In,
    Match,
    Return,
    Break,
    Continue,
    Test,
    Assert,
    And,
    Or,
    True,
    False,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Bang,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    ColonColon,
    Dot,
    DotDot,
    DotDotEq,
    FatArrow,
    Arrow,
    Amp,

    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    Lexer {
        src: source.as_bytes(),
        pos: 0,
        tokens: Vec::new(),
        indents: vec![0],
        depth: 0,
        at_line_start: true,
        line_has_tokens: false,
        pending_newline: false,
    }
    .run()
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
    indents: Vec<usize>,
    /// Nesting depth of `(` and `[`. Newlines inside brackets are ignored.
    depth: usize,
    at_line_start: bool,
    line_has_tokens: bool,
    /// A logical line ended; its `Newline` is emitted once we know the next
    /// line isn't a `.` continuation.
    pending_newline: bool,
}

impl<'a> Lexer<'a> {
    fn run(mut self) -> Result<Vec<Token>> {
        loop {
            if self.at_line_start && self.depth == 0 {
                if !self.line_start()? {
                    break;
                }
                continue;
            }
            let start = self.pos;
            let Some(b) = self.peek() else { break };
            match b {
                b' ' | b'\t' | b'\r' => {
                    self.pos += 1;
                }
                b'\n' => {
                    self.pos += 1;
                    if self.depth == 0 {
                        if self.line_has_tokens {
                            self.pending_newline = true;
                            self.line_has_tokens = false;
                        }
                        self.at_line_start = true;
                    }
                }
                b'#' => self.skip_comment(),
                b'0'..=b'9' => self.number(start)?,
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.ident(start),
                b'"' => self.string(start)?,
                b'\'' => self.char_lit(start)?,
                _ => self.operator(start, b)?,
            }
        }
        if self.pending_newline || self.line_has_tokens {
            self.push_raw(Tok::Newline, self.pos, self.pos);
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            self.push_raw(Tok::Dedent, self.pos, self.pos);
        }
        self.push_raw(Tok::Eof, self.pos, self.pos);
        Ok(self.tokens)
    }

    /// Handles indentation at the start of a line. Returns false at the end of input.
    fn line_start(&mut self) -> Result<bool> {
        let begin = self.pos;
        let mut width = 0;
        while let Some(b) = self.peek() {
            match b {
                b' ' => width += 1,
                b'\t' => {
                    return Err(Error::lex("tab character in indentation (indent with spaces)", Span::new(self.pos, self.pos + 1)));
                }
                _ => break,
            }
            self.pos += 1;
        }
        match self.peek() {
            None => return Ok(false),
            Some(b'\n') => {
                self.pos += 1;
                return Ok(true);
            }
            Some(b'\r') if self.peek_at(1) == Some(b'\n') => {
                self.pos += 2;
                return Ok(true);
            }
            Some(b'#') => {
                self.skip_comment();
                return Ok(true);
            }
            Some(b'.') if self.peek_at(1) != Some(b'.') => {
                // `.method()` continues the previous line's expression.
                self.pending_newline = false;
                self.at_line_start = false;
                self.line_has_tokens = true;
                return Ok(true);
            }
            _ => {}
        }
        if self.pending_newline {
            self.pending_newline = false;
            self.push_raw(Tok::Newline, begin, begin);
        }
        let top = *self.indents.last().unwrap();
        if width > top {
            self.indents.push(width);
            self.push_raw(Tok::Indent, begin, self.pos);
        } else if width < top {
            while width < *self.indents.last().unwrap() {
                self.indents.pop();
                self.push_raw(Tok::Dedent, self.pos, self.pos);
            }
            if width != *self.indents.last().unwrap() {
                return Err(Error::lex("this line's indentation doesn't match any outer block", Span::new(begin, self.pos.max(begin + 1))));
            }
        }
        self.at_line_start = false;
        Ok(true)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos + n).copied()
    }

    fn push_raw(&mut self, tok: Tok, start: usize, end: usize) {
        self.tokens.push(Token { tok, span: Span::new(start, end) });
    }

    fn push(&mut self, tok: Tok, start: usize) {
        self.line_has_tokens = true;
        self.push_raw(tok, start, self.pos);
    }

    fn skip_comment(&mut self) {
        while self.peek().is_some_and(|b| b != b'\n') {
            self.pos += 1;
        }
    }

    fn number(&mut self, start: usize) -> Result<()> {
        while self.peek().is_some_and(|b| b.is_ascii_digit() || b == b'_') {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
            return Err(Error::lex("floats are not supported by the reference interpreter yet", Span::new(start, self.pos + 1)));
        }
        let text: String = std::str::from_utf8(&self.src[start..self.pos]).unwrap().replace('_', "");
        let n = text
            .parse::<i64>()
            .map_err(|_| Error::lex("integer literal is too large for int (i64)", Span::new(start, self.pos)))?;
        self.push(Tok::Int(n), start);
        Ok(())
    }

    fn ident(&mut self, start: usize) {
        while self.peek().is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_') {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let tok = match text {
            "let" => Tok::Let,
            "fun" => Tok::Fun,
            "memo" => Tok::Memo,
            "inout" => Tok::Inout,
            "if" => Tok::If,
            "then" => Tok::Then,
            "else" => Tok::Else,
            "while" => Tok::While,
            "for" => Tok::For,
            "in" => Tok::In,
            "match" => Tok::Match,
            "return" => Tok::Return,
            "break" => Tok::Break,
            "continue" => Tok::Continue,
            "test" => Tok::Test,
            "assert" => Tok::Assert,
            "and" => Tok::And,
            "or" => Tok::Or,
            "true" => Tok::True,
            "false" => Tok::False,
            _ => Tok::Ident(text.into()),
        };
        self.push(tok, start);
    }

    fn escape(&mut self, start: usize) -> Result<u8> {
        // The backslash has been consumed.
        let b = self.peek().ok_or_else(|| Error::lex("unfinished escape sequence", Span::new(start, self.pos)))?;
        self.pos += 1;
        Ok(match b {
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'0' => 0,
            b'\\' => b'\\',
            b'"' => b'"',
            b'\'' => b'\'',
            _ => return Err(Error::lex(format!("unknown escape `\\{}`", b as char), Span::new(self.pos - 2, self.pos))),
        })
    }

    fn string(&mut self, start: usize) -> Result<()> {
        if self.peek_at(1) == Some(b'"') && self.peek_at(2) == Some(b'"') {
            return self.triple_string(start);
        }
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return Err(Error::lex("unterminated string", Span::new(start, self.pos))),
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    self.pos += 1;
                    out.push(self.escape(start)?);
                }
                Some(b) => {
                    out.push(b);
                    self.pos += 1;
                }
            }
        }
        let s = String::from_utf8(out).unwrap();
        self.push(Tok::Str(s.into()), start);
        Ok(())
    }

    /// `"""…"""` with Swift's rules: drop the newline after the opening
    /// quotes and before the closing ones, and dedent every line by the
    /// indentation of the closing quotes.
    fn triple_string(&mut self, start: usize) -> Result<()> {
        self.pos += 3;
        let content_start = self.pos;
        let close = loop {
            if self.pos + 3 > self.src.len() {
                return Err(Error::lex("unterminated triple-quoted string", Span::new(start, start + 3)));
            }
            if self.src[self.pos..].starts_with(b"\"\"\"") {
                break self.pos;
            }
            self.pos += 1;
        };
        self.pos = close + 3;
        let raw = std::str::from_utf8(&self.src[content_start..close]).unwrap().replace("\r\n", "\n");
        let mut body: &str = &raw;
        if let Some(rest) = body.strip_prefix('\n') {
            body = rest;
        }
        let (body, indent) = match body.rfind('\n') {
            Some(i) if body[i + 1..].bytes().all(|b| b == b' ') => (&body[..i], body.len() - i - 1),
            _ => (body, 0),
        };
        let lines: Vec<&str> = body
            .split('\n')
            .map(|line| {
                let spaces = line.bytes().take(indent).take_while(|&b| b == b' ').count();
                &line[spaces..]
            })
            .collect();
        let s = lines.join("\n");
        self.push(Tok::Str(s.into()), start);
        Ok(())
    }

    fn char_lit(&mut self, start: usize) -> Result<()> {
        self.pos += 1;
        let c = match self.peek() {
            Some(b'\\') => {
                self.pos += 1;
                self.escape(start)?
            }
            Some(b) if b != b'\'' && b != b'\n' => {
                self.pos += 1;
                b
            }
            _ => return Err(Error::lex("empty or broken char literal", Span::new(start, self.pos))),
        };
        if self.peek() != Some(b'\'') {
            return Err(Error::lex("char literal must contain exactly one character (use \"…\" for strings)", Span::new(start, self.pos)));
        }
        self.pos += 1;
        self.push(Tok::Char(c), start);
        Ok(())
    }

    fn operator(&mut self, start: usize, b: u8) -> Result<()> {
        let next = self.peek_at(1);
        let next2 = self.peek_at(2);
        let (tok, len) = match (b, next) {
            (b'=', Some(b'=')) => (Tok::EqEq, 2),
            (b'=', Some(b'>')) => (Tok::FatArrow, 2),
            (b'!', Some(b'=')) => (Tok::NotEq, 2),
            (b'<', Some(b'=')) => (Tok::Le, 2),
            (b'>', Some(b'=')) => (Tok::Ge, 2),
            (b'+', Some(b'=')) => (Tok::PlusEq, 2),
            (b'-', Some(b'=')) => (Tok::MinusEq, 2),
            (b'-', Some(b'>')) => (Tok::Arrow, 2),
            (b'*', Some(b'=')) => (Tok::StarEq, 2),
            (b'/', Some(b'=')) => (Tok::SlashEq, 2),
            (b'%', Some(b'=')) => (Tok::PercentEq, 2),
            (b':', Some(b':')) => (Tok::ColonColon, 2),
            (b'.', Some(b'.')) if next2 == Some(b'=') => (Tok::DotDotEq, 3),
            (b'.', Some(b'.')) => (Tok::DotDot, 2),
            (b'=', _) => (Tok::Assign, 1),
            (b'!', _) => (Tok::Bang, 1),
            (b'<', _) => (Tok::Lt, 1),
            (b'>', _) => (Tok::Gt, 1),
            (b'+', _) => (Tok::Plus, 1),
            (b'-', _) => (Tok::Minus, 1),
            (b'*', _) => (Tok::Star, 1),
            (b'/', _) => (Tok::Slash, 1),
            (b'%', _) => (Tok::Percent, 1),
            (b'(', _) => (Tok::LParen, 1),
            (b')', _) => (Tok::RParen, 1),
            (b'[', _) => (Tok::LBracket, 1),
            (b']', _) => (Tok::RBracket, 1),
            (b',', _) => (Tok::Comma, 1),
            (b':', _) => (Tok::Colon, 1),
            (b'.', _) => (Tok::Dot, 1),
            (b'&', _) => (Tok::Amp, 1),
            _ if b >= 128 => {
                let mut end = self.pos + 1;
                while end < self.src.len() && self.src[end] & 0xC0 == 0x80 {
                    end += 1;
                }
                return Err(Error::lex(
                    "non-ASCII character outside a string (a non-breaking space? source files must be ASCII)",
                    Span::new(start, end),
                ));
            }
            _ => return Err(Error::lex(format!("unexpected character `{}`", b as char), Span::new(start, start + 1))),
        };
        self.pos += len;
        match tok {
            Tok::LParen | Tok::LBracket => self.depth += 1,
            Tok::RParen | Tok::RBracket => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        self.push(tok, start);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Tok::*;

    fn kinds(src: &str) -> Vec<Tok> {
        tokenize(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn indentation() {
        assert_eq!(
            kinds("1\n  2\n  3\n    4\n5\n"),
            vec![Int(1), Newline, Indent, Int(2), Newline, Int(3), Newline, Indent, Int(4), Newline, Dedent, Dedent, Int(5), Newline, Eof]
        );
    }

    #[test]
    fn continuation_and_brackets() {
        assert_eq!(
            kinds("a\n  .b(1,\n 2)\nc"),
            vec![
                Ident("a".into()), Dot, Ident("b".into()), LParen, Int(1), Comma, Int(2), RParen, Newline,
                Ident("c".into()), Newline, Eof
            ]
        );
    }

    #[test]
    fn triple_quoted() {
        let toks = kinds("test \"\"\"\n     ab\n     c\n     \"\"\" == 1\n");
        assert_eq!(toks[1], Str("ab\nc".into()));
    }

    #[test]
    fn nbsp_is_an_error() {
        assert!(tokenize("a\u{a0}+ 1").is_err());
    }
}
