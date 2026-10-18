//! The lexer turns source text into a flat list of tokens.
//!
//! eggnog++ source is ASCII (see SPEC.md), so the lexer works on bytes.

use crate::error::{Error, Span};
use crate::token::{Token, TokenKind};
use crate::token::TokenKind::Newline;

pub fn tokenize(source: &str) -> Result<Vec<Token>, Error> {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    src: &'a [u8],
    /// Index of the next byte to read.
    pos: usize,
    tokens: Vec<Token>,
}

// Remove this allow once the lexer uses all the helpers.
#[allow(dead_code)]
impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Lexer { src: source.as_bytes(), pos: 0, tokens: Vec::new() }
    }

    fn run(mut self) -> Result<Vec<Token>, Error> {
        // TODO(you): the main loop. Look at `self.peek()`, decide what kind
        // of token starts here, consume it, and `self.push(...)` it.
        // Finish with an `Eof` token. GUIDE.md walks through it.
        let mut indent_depth = 0;
        while self.pos < self.src.len() {
            let start = self.pos;
            let mut end = self.pos;
            let b = self.src[self.pos];
            
            if b == b'\n' {
                self.tokens.push(Token { kind: Newline, span: Span { start, end } });
                self.pos += 1;
                continue;
            }
            todo!("lext rest")
        }
        todo!("lexer main loop")
    }

    // ---- helpers ----------------------------------------------------------

    fn at_end(&self) -> bool {
        self.pos >= self.src.len()
    }

    /// The next byte without consuming it, or `None` at the end.
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// The byte after the next one. Handy for two-char operators like `==`.
    fn peek_next(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    /// Consumes and returns the next byte.
    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    /// Records a token that started at byte `start` and ends at `self.pos`.
    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token { kind, span: Span::new(start, self.pos) });
    }

    /// The source text from `start` to the current position.
    fn text_from(&self, start: usize) -> &'a str {
        // Safe to unwrap: the input came from a &str and we only split at ASCII bytes.
        std::str::from_utf8(&self.src[start..self.pos]).unwrap()
    }

    fn error(&self, message: impl Into<String>, start: usize) -> Error {
        Error::lex(message, Span::new(start, self.pos.max(start + 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    // These tests are your first target. They fail until `run` is written.

    #[test]
    fn arithmetic() {
        assert_eq!(
            kinds("1 + 2 * (30 - 4)\n"),
            vec![Int(1), Plus, Int(2), Star, LParen, Int(30), Minus, Int(4), RParen, Newline, Eof]
        );
    }

    #[test]
    fn spans_point_at_the_source() {
        let tokens = tokenize("12 + 3").unwrap();
        assert_eq!(tokens[0].span, Span::new(0, 2));
        assert_eq!(tokens[1].span, Span::new(3, 4));
        assert_eq!(tokens[2].span, Span::new(5, 6));
    }

    #[test]
    fn missing_final_newline_still_ends_the_line() {
        assert_eq!(kinds("7"), vec![Int(7), Newline, Eof]);
    }

    #[test]
    fn blank_lines_and_comments_produce_nothing() {
        assert_eq!(kinds("\n# hello\n\n1\n"), vec![Int(1), Newline, Eof]);
    }

    #[test]
    fn indent_and_dedent() {
        // Pretend these are lines of a block; the numbers stand in for code.
        let src = "1\n  2\n  3\n    4\n5\n";
        assert_eq!(
            kinds(src),
            vec![
                Int(1), Newline,
                Indent, Int(2), Newline,
                Int(3), Newline,
                Indent, Int(4), Newline,
                Dedent, Dedent, Int(5), Newline,
                Eof,
            ]
        );
    }

    #[test]
    fn blocks_still_open_at_the_end_are_closed() {
        assert_eq!(kinds("1\n  2"), vec![Int(1), Newline, Indent, Int(2), Newline, Dedent, Eof]);
    }

    #[test]
    fn dedent_must_match_an_outer_level() {
        assert!(tokenize("1\n    2\n  3\n").is_err());
    }

    #[test]
    fn tabs_are_an_error() {
        assert!(tokenize("1\n\t2\n").is_err());
    }

    #[test]
    fn integer_too_large_is_an_error() {
        assert!(tokenize("99999999999999999999\n").is_err());
    }
}
