//! The parser turns the token list into an AST.
//!
//! Recommended technique: recursive descent for statements, and precedence
//! climbing (a.k.a. Pratt parsing) for expressions. GUIDE.md explains both.

use crate::ast::Program;
use crate::error::Error;
use crate::token::Token;

pub fn parse(tokens: &[Token]) -> Result<Program, Error> {
    let _ = tokens;
    todo!("parser")
}
