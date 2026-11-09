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
pub mod types;
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

/// Runs a program as a puzzle solution: its answer must be an `int` or a
/// `string` (SPEC: Program structure). `run_program` itself allows any value,
/// so the golden tests can show tuples and lists.
pub fn run_solution(program: &Program, input: &str, base_dir: &Path) -> Result<Value> {
    let answer = run_program(program, input, base_dir)?;
    if matches!(answer, Value::Int(_) | Value::Str(_)) {
        return Ok(answer);
    }
    let span = program.body.iter().rev().find(|s| !interp::is_test(s)).map_or(Span::default(), |s| s.span);
    let hint = match answer {
        Value::List(_) => "; to join numbers, use `.to_string(sep = \",\")`",
        _ => "",
    };
    Err(Error::runtime(format!("the answer must be an int or a string, found {}{hint}", interp::describe(&answer)), span))
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
                    let actual = run_solution(program, &input, base_dir).map_err(TestFailure::Error)?;
                    if actual == expected { Ok(()) } else { Err(TestFailure::Wrong { expected, actual }) }
                })();
                results.push(TestResult { span: stmt.span, outcome });
            }
            StmtKind::TestBlock { body, .. } => {
                let outcome = (|| {
                    let mut it = Interp::new("", base_dir.to_path_buf());
                    for s in &program.body {
                        if let StmtKind::Let(pat, _) = &s.kind {
                            pattern_names(pat, &mut it.script_vars);
                        }
                    }
                    it.declare_imports(program).map_err(TestFailure::Error)?;
                    it.declare_types(program).map_err(TestFailure::Error)?;
                    // Only the types and functions are defined; the script itself doesn't run.
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

fn pattern_names(p: &ast::Pattern, out: &mut std::collections::HashSet<ast::Name>) {
    match p {
        ast::Pattern::Name(n) => {
            out.insert(n.clone());
        }
        ast::Pattern::Wild => {}
        ast::Pattern::Tuple(ps) | ast::Pattern::List(ps) => ps.iter().for_each(|p| pattern_names(p, out)),
    }
}
