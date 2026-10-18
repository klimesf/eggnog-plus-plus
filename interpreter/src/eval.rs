//! The evaluator walks the AST and computes values: a tree-walking interpreter.
//!
//! The interpreter is dynamically typed: values carry their type at runtime
//! and type errors are reported when the offending code runs. The static
//! type checker comes later, with the compiler (PLAN.md step 3).

use std::fmt;

use crate::ast::Program;
use crate::error::Error;

/// A runtime value. Grow it as the language grows: `Bool`, `Char`, `Str`,
/// `Tuple`, `List`, `Grid`, …
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
}

/// Runs a program and returns the value of its last expression (the answer).
pub fn run(program: &Program, input: &str) -> Result<Value, Error> {
    let _ = (program, input);
    todo!("evaluator")
}

/// How an answer is printed by `eggnog run`.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
        }
    }
}
