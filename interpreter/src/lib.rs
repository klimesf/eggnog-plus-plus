//! eggnog++ tree-walking interpreter.
//!
//! Pipeline: source text → `lexer` → tokens → `parser` → AST → `eval` → value.

pub mod ast;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod token;

use error::Error;
use eval::Value;

/// Runs a whole program: lex, parse, evaluate.
pub fn run_source(source: &str, input: &str) -> Result<Value, Error> {
    let tokens = lexer::tokenize(source)?;
    let program = parser::parse(&tokens)?;
    eval::run(&program, input)
}
