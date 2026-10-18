//! Tokens: the output of the lexer and the input of the parser.

use crate::error::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The kinds of token the lexer can produce.
///
/// This is only the starter set for the first milestone (integer
/// arithmetic, see `GUIDE.md`). Add variants as you grow the language:
/// identifiers and keywords, `:`, comparison operators, strings, chars, …
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// An integer literal, already converted: `42` → `Int(42)`.
    Int(i64),

    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,

    /// The end of a logical line. Blank lines and comment-only lines
    /// produce no `Newline`.
    Newline,
    /// The line is indented deeper than the previous one: a block starts.
    Indent,
    /// The line is indented less: one `Dedent` per closed block.
    Dedent,
    /// Always the last token.
    Eof,
}
