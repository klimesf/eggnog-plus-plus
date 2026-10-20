//! Tokens → AST. Recursive descent for statements, precedence climbing
//! (one function per level) for expressions.

use std::rc::Rc;

use crate::ast::*;
use crate::error::{Error, Result, Span};
use crate::lexer::{Tok, Token};

pub fn parse(tokens: &[Token]) -> Result<Program> {
    let mut p = Parser { t: tokens, i: 0 };
    let mut body = Vec::new();
    while !p.check(&Tok::Eof) {
        if p.eat(&Tok::Newline) {
            continue;
        }
        body.push(p.stmt()?);
    }
    Ok(Program { body })
}

struct Parser<'a> {
    t: &'a [Token],
    i: usize,
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Int(n) => format!("`{n}`"),
        Tok::Str(_) => "a string".into(),
        Tok::Char(_) => "a char".into(),
        Tok::Ident(n) => format!("`{n}`"),
        Tok::Newline => "end of line".into(),
        Tok::Indent => "an indented block".into(),
        Tok::Dedent => "end of block".into(),
        Tok::Eof => "end of file".into(),
        other => format!("`{}`", format!("{other:?}").to_lowercase()),
    }
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.t[self.i].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        &self.t[(self.i + n).min(self.t.len() - 1)].tok
    }

    fn span(&self) -> Span {
        self.t[self.i].span
    }

    fn prev_span(&self) -> Span {
        self.t[self.i.saturating_sub(1)].span
    }

    fn advance(&mut self) -> &Token {
        let t = &self.t[self.i];
        if self.i < self.t.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn check(&self, t: &Tok) -> bool {
        self.peek() == t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.check(t) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &Tok, what: &str) -> Result<Span> {
        if self.check(t) {
            Ok(self.advance().span)
        } else {
            Err(Error::parse(format!("expected {what}, found {}", describe(self.peek())), self.span()))
        }
    }

    fn ident(&mut self, what: &str) -> Result<Name> {
        match self.peek().clone() {
            Tok::Ident(n) => {
                self.advance();
                Ok(n)
            }
            other => Err(Error::parse(format!("expected {what}, found {}", describe(&other)), self.span())),
        }
    }

    /// End of a simple statement: a newline, or the end of the enclosing block.
    fn end_stmt(&mut self) -> Result<()> {
        if self.i > 0 && self.t[self.i - 1].tok == Tok::Dedent {
            return Ok(()); // the statement ended with a block (e.g. a `match` expression)
        }
        if self.eat(&Tok::Newline) || self.check(&Tok::Dedent) || self.check(&Tok::Eof) {
            Ok(())
        } else {
            Err(Error::parse(format!("expected end of line, found {}", describe(self.peek())), self.span()))
        }
    }

    // ---- statements --------------------------------------------------------

    fn stmt(&mut self) -> Result<Stmt> {
        let start = self.span();
        let kind = match self.peek() {
            Tok::Let => {
                self.advance();
                let pat = self.pattern()?;
                self.expect(&Tok::Assign, "`=`")?;
                let value = self.expr()?;
                self.end_stmt()?;
                StmtKind::Let(pat, value)
            }
            Tok::If => return self.if_stmt(),
            Tok::While => {
                self.advance();
                let cond = self.cond()?;
                let body = self.block()?;
                StmtKind::While { cond, body }
            }
            Tok::For => {
                self.advance();
                let pat = self.pattern()?;
                self.expect(&Tok::In, "`in`")?;
                let iter = self.expr()?;
                let body = self.block()?;
                StmtKind::For { pat, iter, body }
            }
            Tok::Match => {
                self.advance();
                let subject = self.expr()?;
                self.expect(&Tok::Colon, "`:`")?;
                self.expect(&Tok::Newline, "a new line after `match …:`")?;
                self.expect(&Tok::Indent, "indented match arms")?;
                let mut arms = Vec::new();
                while !self.eat(&Tok::Dedent) {
                    if self.eat(&Tok::Newline) {
                        continue;
                    }
                    let pat = self.match_pat()?;
                    let body = self.block()?;
                    arms.push((pat, body));
                }
                StmtKind::Match { subject, arms }
            }
            Tok::Fun | Tok::Memo => StmtKind::Fun(Rc::new(self.fun()?)),
            Tok::Return => {
                self.advance();
                let value = if matches!(self.peek(), Tok::Newline | Tok::Dedent | Tok::Eof) { None } else { Some(self.expr()?) };
                self.end_stmt()?;
                StmtKind::Return(value)
            }
            Tok::Break => {
                self.advance();
                self.end_stmt()?;
                StmtKind::Break
            }
            Tok::Continue => {
                self.advance();
                self.end_stmt()?;
                StmtKind::Continue
            }
            Tok::Test => {
                self.advance();
                if let (Tok::Str(name), Tok::Colon) = (self.peek().clone(), self.peek_at(1)) {
                    self.advance();
                    let body = self.block()?;
                    StmtKind::TestBlock { name, body }
                } else {
                    let e = self.expr()?;
                    self.end_stmt()?;
                    match e.kind {
                        ExprKind::Binary(BinOp::Eq, input, expected) => StmtKind::Test { input: *input, expected: *expected },
                        _ => return Err(Error::parse("a test must look like `test <input> == <expected answer>`", e.span)),
                    }
                }
            }
            Tok::Assert => {
                self.advance();
                let e = self.expr()?;
                self.end_stmt()?;
                StmtKind::Assert(e)
            }
            _ => {
                let target = self.expr()?;
                let op = match self.peek() {
                    Tok::Assign => Some(None),
                    Tok::PlusEq => Some(Some(BinOp::Add)),
                    Tok::MinusEq => Some(Some(BinOp::Sub)),
                    Tok::StarEq => Some(Some(BinOp::Mul)),
                    Tok::SlashEq => Some(Some(BinOp::Div)),
                    Tok::PercentEq => Some(Some(BinOp::Rem)),
                    _ => None,
                };
                if let Some(op) = op {
                    self.advance();
                    if !matches!(target.kind, ExprKind::Var(_) | ExprKind::Index { .. }) {
                        return Err(Error::parse("can only assign to a variable or an indexed element", target.span));
                    }
                    let value = self.expr()?;
                    self.end_stmt()?;
                    StmtKind::Assign { target, op, value }
                } else {
                    self.end_stmt()?;
                    StmtKind::Expr(target)
                }
            }
        };
        Ok(Stmt { kind, span: start.to(self.prev_span()) })
    }

    fn if_stmt(&mut self) -> Result<Stmt> {
        let start = self.expect(&Tok::If, "`if`")?;
        let cond = self.cond()?;
        let then = self.block()?;
        let els = if self.eat(&Tok::Else) {
            if self.check(&Tok::If) {
                Some(vec![self.if_stmt()?])
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        Ok(Stmt { kind: StmtKind::If { cond, then, els }, span: start.to(self.prev_span()) })
    }

    fn cond(&mut self) -> Result<Cond> {
        let e = self.expr()?;
        if self.eat(&Tok::Assign) {
            let pat = expr_to_pattern(e)?;
            let value = self.expr()?;
            Ok(Cond::Bind(pat, value))
        } else {
            Ok(Cond::Expr(e))
        }
    }

    /// `: stmt` on the same line, or `:` NEWLINE INDENT stmts DEDENT.
    fn block(&mut self) -> Result<Vec<Stmt>> {
        self.expect(&Tok::Colon, "`:`")?;
        if self.eat(&Tok::Newline) {
            self.expect(&Tok::Indent, "an indented block")?;
            let mut body = Vec::new();
            while !self.eat(&Tok::Dedent) {
                if self.eat(&Tok::Newline) {
                    continue;
                }
                body.push(self.stmt()?);
            }
            Ok(body)
        } else {
            Ok(vec![self.stmt()?])
        }
    }

    fn match_pat(&mut self) -> Result<MatchPat> {
        if let Tok::Ident(n) = self.peek() {
            if &**n == "_" {
                self.advance();
                return Ok(MatchPat::Wild);
            }
        }
        Ok(MatchPat::Value(self.or()?))
    }

    fn fun(&mut self) -> Result<FunDecl> {
        let start = self.span();
        let memo = self.eat(&Tok::Memo);
        self.expect(&Tok::Fun, "`fun`")?;
        let name = self.ident("a function name")?;
        self.expect(&Tok::LParen, "`(`")?;
        let mut params = Vec::new();
        let mut seen_default = false;
        while !self.eat(&Tok::RParen) {
            let inout = self.eat(&Tok::Inout);
            let pspan = self.span();
            let pname = self.ident("a parameter name")?;
            if !self.eat(&Tok::Colon) {
                return Err(Error::parse(format!("parameter `{pname}` needs a type (`{pname}: int`)"), pspan));
            }
            self.skip_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            if default.is_some() {
                seen_default = true;
            } else if seen_default {
                return Err(Error::parse("parameters with defaults must come after those without", pspan));
            }
            params.push(Param { name: pname, inout, default });
            if !self.eat(&Tok::Comma) {
                self.expect(&Tok::RParen, "`,` or `)`")?;
                break;
            }
        }
        if self.eat(&Tok::Arrow) {
            self.skip_type()?;
        }
        let body = self.block()?;
        Ok(FunDecl { name, params, memo, body, span: start })
    }

    /// Types are parsed and ignored: the reference interpreter is dynamically typed.
    fn skip_type(&mut self) -> Result<()> {
        if self.eat(&Tok::LParen) {
            while !self.eat(&Tok::RParen) {
                self.skip_type()?;
                if !self.eat(&Tok::Comma) {
                    self.expect(&Tok::RParen, "`)` in a tuple type")?;
                    break;
                }
            }
            return Ok(());
        }
        self.ident("a type")?;
        if self.eat(&Tok::Lt) {
            loop {
                self.skip_type()?;
                if self.eat(&Tok::Gt) {
                    break;
                }
                self.expect(&Tok::Comma, "`,` or `>` in a type")?;
            }
        }
        Ok(())
    }

    fn pattern(&mut self) -> Result<Pattern> {
        match self.peek().clone() {
            Tok::Ident(n) => {
                self.advance();
                Ok(if &*n == "_" { Pattern::Wild } else { Pattern::Name(n) })
            }
            Tok::LParen | Tok::LBracket => {
                let list = self.check(&Tok::LBracket);
                let close = if list { Tok::RBracket } else { Tok::RParen };
                self.advance();
                let mut items = Vec::new();
                while !self.eat(&close) {
                    items.push(self.pattern()?);
                    if !self.eat(&Tok::Comma) {
                        self.expect(&close, "`,` or a closing bracket")?;
                        break;
                    }
                }
                Ok(if list { Pattern::List(items) } else { Pattern::Tuple(items) })
            }
            other => Err(Error::parse(format!("expected a name or a destructuring pattern, found {}", describe(&other)), self.span())),
        }
    }

    // ---- expressions -------------------------------------------------------

    fn expr(&mut self) -> Result<Expr> {
        let start = self.span();
        match self.peek() {
            Tok::Ident(_) if self.peek_at(1) == &Tok::FatArrow => {
                let p = self.pattern()?;
                self.advance();
                let body = self.expr()?;
                let span = start.to(body.span);
                return Ok(Expr { kind: ExprKind::Lambda(Rc::new(Lambda { params: vec![p], body })), span });
            }
            Tok::LParen if self.parens_then_arrow() => {
                self.advance();
                let mut params = Vec::new();
                while !self.eat(&Tok::RParen) {
                    params.push(self.pattern()?);
                    if self.eat(&Tok::Colon) {
                        self.skip_type()?;
                    }
                    if !self.eat(&Tok::Comma) {
                        self.expect(&Tok::RParen, "`,` or `)`")?;
                        break;
                    }
                }
                self.expect(&Tok::FatArrow, "`=>`")?;
                let body = self.expr()?;
                let span = start.to(body.span);
                return Ok(Expr { kind: ExprKind::Lambda(Rc::new(Lambda { params, body })), span });
            }
            Tok::If => {
                self.advance();
                let c = self.or()?;
                if self.check(&Tok::Colon) {
                    return Err(Error::parse("an `if` inside an expression needs `then … else …`", self.span()));
                }
                self.expect(&Tok::Then, "`then`")?;
                let a = self.expr()?;
                self.expect(&Tok::Else, "`else` (an if-expression always needs one)")?;
                let b = self.expr()?;
                let span = start.to(b.span);
                return Ok(Expr { kind: ExprKind::IfElse(Box::new(c), Box::new(a), Box::new(b)), span });
            }
            Tok::Match => {
                self.advance();
                let subject = self.expr()?;
                self.expect(&Tok::Colon, "`:`")?;
                self.expect(&Tok::Newline, "a new line after `match …:`")?;
                self.expect(&Tok::Indent, "indented match arms")?;
                let mut arms = Vec::new();
                while !self.eat(&Tok::Dedent) {
                    if self.eat(&Tok::Newline) {
                        continue;
                    }
                    let pat = self.match_pat()?;
                    self.expect(&Tok::Colon, "`:`")?;
                    let value = self.expr()?;
                    self.end_stmt()?;
                    arms.push((pat, value));
                }
                let span = start.to(self.prev_span());
                return Ok(Expr { kind: ExprKind::Match(Box::new(subject), arms), span });
            }
            _ => {}
        }
        self.or()
    }

    /// At `(`: is this the parameter list of a lambda?
    fn parens_then_arrow(&self) -> bool {
        let mut depth = 0;
        let mut j = self.i;
        while j < self.t.len() {
            match self.t[j].tok {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return self.t.get(j + 1).is_some_and(|t| t.tok == Tok::FatArrow);
                    }
                }
                Tok::Newline | Tok::Eof => return false,
                _ => {}
            }
            j += 1;
        }
        false
    }

    fn binary(l: Expr, op: BinOp, r: Expr) -> Expr {
        let span = l.span.to(r.span);
        Expr { kind: ExprKind::Binary(op, Box::new(l), Box::new(r)), span }
    }

    fn or(&mut self) -> Result<Expr> {
        let mut l = self.and()?;
        while self.eat(&Tok::Or) {
            let r = self.and()?;
            let span = l.span.to(r.span);
            l = Expr { kind: ExprKind::Or(Box::new(l), Box::new(r)), span };
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Expr> {
        let mut l = self.cmp()?;
        while self.eat(&Tok::And) {
            let r = self.cmp()?;
            let span = l.span.to(r.span);
            l = Expr { kind: ExprKind::And(Box::new(l), Box::new(r)), span };
        }
        Ok(l)
    }

    fn cmp(&mut self) -> Result<Expr> {
        let l = self.range()?;
        let op = match self.peek() {
            Tok::EqEq => BinOp::Eq,
            Tok::NotEq => BinOp::Ne,
            Tok::Lt => BinOp::Lt,
            Tok::Le => BinOp::Le,
            Tok::Gt => BinOp::Gt,
            Tok::Ge => BinOp::Ge,
            Tok::In => {
                self.advance();
                let r = self.range()?;
                let span = l.span.to(r.span);
                return Ok(Expr { kind: ExprKind::In { item: Box::new(l), coll: Box::new(r), negated: false }, span });
            }
            Tok::Bang if self.peek_at(1) == &Tok::In => {
                self.advance();
                self.advance();
                let r = self.range()?;
                let span = l.span.to(r.span);
                return Ok(Expr { kind: ExprKind::In { item: Box::new(l), coll: Box::new(r), negated: true }, span });
            }
            _ => return Ok(l),
        };
        self.advance();
        let r = self.range()?;
        Ok(Self::binary(l, op, r))
    }

    fn starts_expr(&self) -> bool {
        matches!(
            self.peek(),
            Tok::Int(_) | Tok::Str(_) | Tok::Char(_) | Tok::Ident(_) | Tok::LParen | Tok::LBracket | Tok::Minus | Tok::Bang | Tok::True | Tok::False
        )
    }

    fn range(&mut self) -> Result<Expr> {
        let start_span = self.span();
        let start = if matches!(self.peek(), Tok::DotDot | Tok::DotDotEq) { None } else { Some(self.add()?) };
        let inclusive = match self.peek() {
            Tok::DotDot => false,
            Tok::DotDotEq => true,
            _ => return Ok(start.unwrap()),
        };
        self.advance();
        let end = if self.starts_expr() { Some(Box::new(self.add()?)) } else { None };
        if inclusive && end.is_none() {
            return Err(Error::parse("`..=` needs an end", self.span()));
        }
        let span = start_span.to(self.prev_span());
        Ok(Expr { kind: ExprKind::Range { start: start.map(Box::new), end, inclusive }, span })
    }

    fn add(&mut self) -> Result<Expr> {
        let mut l = self.mul()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(l),
            };
            self.advance();
            let r = self.mul()?;
            l = Self::binary(l, op, r);
        }
    }

    fn mul(&mut self) -> Result<Expr> {
        let mut l = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => return Ok(l),
            };
            self.advance();
            let r = self.unary()?;
            l = Self::binary(l, op, r);
        }
    }

    fn unary(&mut self) -> Result<Expr> {
        let start = self.span();
        let op = match self.peek() {
            Tok::Minus => UnOp::Neg,
            Tok::Bang => UnOp::Not,
            Tok::Amp => {
                self.advance();
                let e = self.unary()?;
                let span = start.to(e.span);
                return Ok(Expr { kind: ExprKind::Ref(Box::new(e)), span });
            }
            _ => return self.postfix(),
        };
        self.advance();
        let e = self.unary()?;
        let span = start.to(e.span);
        Ok(Expr { kind: ExprKind::Unary(op, Box::new(e)), span })
    }

    fn postfix(&mut self) -> Result<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.eat(&Tok::Dot) {
                let name = self.ident("a method name")?;
                if !self.check(&Tok::LParen) {
                    return Err(Error::parse(format!("expected `(` after `.{name}` (there is no field access; methods need parentheses)"), self.span()));
                }
                self.advance();
                let args = self.args()?;
                let span = e.span.to(self.prev_span());
                e = Expr { kind: ExprKind::Method { recv: Box::new(e), name, args }, span };
            } else if self.eat(&Tok::LBracket) {
                let index = self.expr()?;
                self.expect(&Tok::RBracket, "`]`")?;
                let span = e.span.to(self.prev_span());
                e = Expr { kind: ExprKind::Index { recv: Box::new(e), index: Box::new(index) }, span };
            } else {
                return Ok(e);
            }
        }
    }

    /// Arguments after `(`, up to and including `)`.
    fn args(&mut self) -> Result<Vec<Arg>> {
        let mut args: Vec<Arg> = Vec::new();
        while !self.eat(&Tok::RParen) {
            let named = matches!(self.peek(), Tok::Ident(_)) && self.peek_at(1) == &Tok::Assign;
            let name = if named {
                let n = self.ident("an argument name")?;
                self.advance();
                Some(n)
            } else {
                None
            };
            let value = self.expr()?;
            if name.is_none() && args.iter().any(|a| a.name.is_some()) {
                return Err(Error::parse("positional arguments must come before named ones", value.span));
            }
            args.push(Arg { name, value });
            if !self.eat(&Tok::Comma) {
                self.expect(&Tok::RParen, "`,` or `)`")?;
                break;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr> {
        let start = self.span();
        let tok = self.peek().clone();
        let kind = match tok {
            Tok::Int(n) => {
                self.advance();
                ExprKind::Int(n)
            }
            Tok::Str(s) => {
                self.advance();
                ExprKind::Str(s)
            }
            Tok::Char(c) => {
                self.advance();
                ExprKind::Char(c)
            }
            Tok::True | Tok::False => {
                self.advance();
                ExprKind::Bool(tok == Tok::True)
            }
            Tok::Ident(name) => {
                self.advance();
                if self.eat(&Tok::ColonColon) {
                    let member = self.ident("a name after `::`")?;
                    ExprKind::Path(name, member)
                } else if self.eat(&Tok::LParen) {
                    let args = self.args()?;
                    ExprKind::Call { name, args }
                } else {
                    ExprKind::Var(name)
                }
            }
            Tok::LParen => {
                self.advance();
                if self.eat(&Tok::RParen) {
                    ExprKind::Tuple(Vec::new())
                } else {
                    let first = self.expr()?;
                    if self.eat(&Tok::RParen) {
                        return Ok(Expr { kind: first.kind, span: start.to(self.prev_span()) });
                    }
                    let mut items = vec![first];
                    while self.eat(&Tok::Comma) {
                        if self.check(&Tok::RParen) {
                            break;
                        }
                        items.push(self.expr()?);
                    }
                    self.expect(&Tok::RParen, "`)`")?;
                    ExprKind::Tuple(items)
                }
            }
            Tok::LBracket => {
                self.advance();
                let mut items = Vec::new();
                while !self.eat(&Tok::RBracket) {
                    items.push(self.expr()?);
                    if !self.eat(&Tok::Comma) {
                        self.expect(&Tok::RBracket, "`,` or `]`")?;
                        break;
                    }
                }
                ExprKind::List(items)
            }
            Tok::Then => return Err(Error::parse("unexpected `then` (if-expressions are `if c then a else b`)", start)),
            other => return Err(Error::parse(format!("expected an expression, found {}", describe(&other)), start)),
        };
        Ok(Expr { kind, span: start.to(self.prev_span()) })
    }
}

fn expr_to_pattern(e: Expr) -> Result<Pattern> {
    match e.kind {
        ExprKind::Var(n) => Ok(if &*n == "_" { Pattern::Wild } else { Pattern::Name(n) }),
        ExprKind::Tuple(items) => Ok(Pattern::Tuple(items.into_iter().map(expr_to_pattern).collect::<Result<_>>()?)),
        ExprKind::List(items) => Ok(Pattern::List(items.into_iter().map(expr_to_pattern).collect::<Result<_>>()?)),
        _ => Err(Error::parse("the left side of `=` in a condition must be a name or a destructuring pattern (did you mean `==`?)", e.span)),
    }
}
