//! The abstract syntax tree: the output of the parser and the input of
//! the evaluator.
//!
//! This is a deliberately tiny seed for the first milestone. Grow it as you
//! add syntax: `Stmt::Let`, `Stmt::While`, `Expr::Var`, `Expr::Call`, …
//! Keep a `Span` on every node so runtime errors can point at the source.

use crate::error::Span;

/// A whole `.egg` file: the body of the implicit `solve(input)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// An expression on its own line. The last one is the program's answer.
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    // TODO(you): binary operations (`1 + 2`), unary minus, …
}
