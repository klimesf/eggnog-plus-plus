use std::rc::Rc;

use crate::error::Span;

pub type Name = Rc<str>;

#[derive(Debug)]
pub struct Program {
    pub body: Vec<Stmt>,
}

#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StmtKind {
    Let(Pattern, Expr),
    Assign { target: Expr, op: Option<BinOp>, value: Expr },
    Expr(Expr),
    If { cond: Cond, then: Vec<Stmt>, els: Option<Vec<Stmt>> },
    While { cond: Cond, body: Vec<Stmt> },
    For { pat: Pattern, iter: Expr, body: Vec<Stmt> },
    Match { subject: Expr, arms: Vec<(MatchPat, Vec<Stmt>)> },
    Break,
    Continue,
    Return(Option<Expr>),
    Fun(Rc<FunDecl>),
    /// `test <input> == <expected>`
    Test { input: Expr, expected: Expr },
    /// `test "name":` with a block of asserts
    TestBlock { name: Name, body: Vec<Stmt> },
    Assert(Expr),
}

#[derive(Debug)]
pub enum Cond {
    Expr(Expr),
    /// `if pattern = maybe_expr:` / `while pattern = maybe_expr:`
    Bind(Pattern, Expr),
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Name(Name),
    Wild,
    Tuple(Vec<Pattern>),
    List(Vec<Pattern>),
}

#[derive(Debug)]
pub enum MatchPat {
    Wild,
    Value(Expr),
}

#[derive(Debug)]
pub struct FunDecl {
    pub name: Name,
    pub params: Vec<Param>,
    /// `None` when the function returns nothing.
    pub ret: Option<Type>,
    pub memo: bool,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Type,
    pub inout: bool,
    pub default: Option<Expr>,
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Int(i64),
    Str(Rc<str>),
    Char(u8),
    Bool(bool),
    Var(Name),
    /// `int::Max`
    Path(Name, Name),
    Tuple(Vec<Expr>),
    List(Vec<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    In { item: Box<Expr>, coll: Box<Expr>, negated: bool },
    Range { start: Option<Box<Expr>>, end: Option<Box<Expr>>, inclusive: bool },
    Call { name: Name, args: Vec<Arg> },
    Method { recv: Box<Expr>, name: Name, args: Vec<Arg> },
    Index { recv: Box<Expr>, index: Box<Expr> },
    Lambda(Rc<Lambda>),
    IfElse(Box<Expr>, Box<Expr>, Box<Expr>),
    Match(Box<Expr>, Vec<(MatchPat, Expr)>),
    /// `&place`, only valid as an `inout` argument.
    Ref(Box<Expr>),
}

#[derive(Debug)]
pub struct Arg {
    pub name: Option<Name>,
    pub value: Expr,
}

#[derive(Debug)]
pub struct Lambda {
    pub params: Vec<Pattern>,
    pub body: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

/// A type written in a signature. The reference interpreter checks argument
/// and return values against it at runtime (see `types.rs`).
#[derive(Debug, Clone)]
pub enum Type {
    Int,
    Bool,
    Char,
    Str,
    Float,
    List(Box<Type>),
    Tuple(Vec<Type>),
    /// `Set<T>`, `Map<K, V>`, `Queue<T>`, `Stack<T>`, `MinQueue<T>`, `Grid<T>`
    Container(&'static str, Vec<Type>),
}
