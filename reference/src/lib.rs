//! Reference tree-walking interpreter for eggnog++.
//!
//! Its purpose is to let you run the examples and get a feel for the
//! language. It is dynamically typed: types in signatures are parsed and
//! ignored, and type errors show up when the code runs.

pub mod ast;
pub mod builtins;
pub mod error;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod value;

use std::path::Path;

use ast::{Program, StmtKind};
use error::{Error, Result, Span};
use interp::Interp;
use value::Value;

pub fn parse_source(source: &str) -> Result<Program> {
    let tokens = lexer::tokenize(source)?;
    parser::parse(&tokens)
}

/// The runner strips the single trailing newline from the input (SPEC: Input).
pub fn strip_input(input: &str) -> &str {
    input.strip_suffix("\r\n").or_else(|| input.strip_suffix('\n')).unwrap_or(input)
}

pub fn run_program(program: &Program, input: &str, base_dir: &Path) -> Result<Value> {
    Interp::new(strip_input(input), base_dir.to_path_buf()).run(program)
}

pub struct TestResult {
    pub span: Span,
    pub outcome: std::result::Result<(), TestFailure>,
}

pub enum TestFailure {
    /// The program ran but produced a different answer.
    Wrong { expected: Value, actual: Value },
    /// Something failed while running the test.
    Error(Error),
}

/// Runs every `test` statement at the top level of the program.
pub fn run_tests(program: &Program, base_dir: &Path) -> Vec<TestResult> {
    let mut results = Vec::new();
    for stmt in &program.body {
        match &stmt.kind {
            StmtKind::Test { input, expected } => {
                let outcome = (|| {
                    let mut bare = Interp::new("", base_dir.to_path_buf());
                    let input = match bare.eval(input).map_err(TestFailure::Error)? {
                        Value::Str(s) => s,
                        other => {
                            return Err(TestFailure::Error(Error::runtime(
                                format!("a test's input must be a string or file(\"…\"), found a {}", other.type_name()),
                                input.span,
                            )));
                        }
                    };
                    let expected = bare.eval(expected).map_err(TestFailure::Error)?;
                    let actual = run_program(program, &input, base_dir).map_err(TestFailure::Error)?;
                    if actual == expected { Ok(()) } else { Err(TestFailure::Wrong { expected, actual }) }
                })();
                results.push(TestResult { span: stmt.span, outcome });
            }
            StmtKind::TestBlock { body, .. } => {
                let outcome = (|| {
                    let mut it = Interp::new("", base_dir.to_path_buf());
                    // Only the functions are defined; the script itself doesn't run.
                    let funs: Vec<&ast::Stmt> = program.body.iter().filter(|s| matches!(s.kind, StmtKind::Fun(_))).collect();
                    for f in funs {
                        it.exec_stmts(std::slice::from_ref(f)).map_err(TestFailure::Error)?;
                    }
                    it.exec_stmts(body).map_err(TestFailure::Error)?;
                    Ok(())
                })();
                results.push(TestResult { span: stmt.span, outcome });
            }
            _ => {}
        }
    }
    results
}
