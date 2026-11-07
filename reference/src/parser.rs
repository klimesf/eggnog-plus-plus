//! Tokens → AST. Recursive descent for statements, precedence climbing
//! (one function per level) for expressions.

use std::rc::Rc;

use crate::ast::*;
use crate::error::{Error, Result, Span};
use crate::lexer::{Tok, Token};

/// Methods whose arguments may be lambdas, so an argument that uses `it`
/// becomes `it => …`. `it` belongs to the innermost such call.
const LAMBDA_METHODS: &[&str] =
    &["map", "filter", "flat_map", "scan", "count", "any", "all", "find", "find_last", "position", "sort", "to_sorted"];

const IT_RESERVED: &str = "`it` is reserved for the implicit lambda parameter (`.map(it * 2)`); pick another name";

/// Turns an argument that uses `it` (outside nested lambdas) into `it => arg`.
fn wrap_it(e: &mut Expr) {
    if matches!(e.kind, ExprKind::Lambda(_)) || !uses_it(e) {
        return;
    }
    let span = e.span;
    let body = std::mem::replace(e, Expr { kind: ExprKind::Bool(false), span });
    *e = Expr { kind: ExprKind::Lambda(Rc::new(Lambda { params: vec![Pattern::Name("it".into())], body })), span };
}

fn pat_uses_it(p: &MatchPat) -> bool {
    match p {
        MatchPat::Value(e) => uses_it(e),
        MatchPat::Tuple(ps, _) => ps.iter().any(pat_uses_it),
        MatchPat::Wild | MatchPat::Variant { .. } => false,
    }
}

fn uses_it(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Var(n) => &**n == "it",
        ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Char(_) | ExprKind::Bool(_) | ExprKind::Path(..) => false,
        ExprKind::Lambda(_) => false,
        ExprKind::Tuple(xs) | ExprKind::List(xs) => xs.iter().any(uses_it),
        ExprKind::Unary(_, x) | ExprKind::Ref(x) | ExprKind::Cast(x, _) => uses_it(x),
        ExprKind::Binary(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) => uses_it(a) || uses_it(b),
        ExprKind::In { item, coll, .. } => uses_it(item) || uses_it(coll),
        ExprKind::Range { start, end, .. } => start.as_deref().is_some_and(uses_it) || end.as_deref().is_some_and(uses_it),
        ExprKind::Call { args, .. } => args.iter().any(|a| uses_it(&a.value)),
        ExprKind::Method { recv, args, .. } => uses_it(recv) || args.iter().any(|a| uses_it(&a.value)),
        ExprKind::Index { recv, index } => uses_it(recv) || uses_it(index),
        ExprKind::Field { recv, .. } => uses_it(recv),
        ExprKind::IfElse(c, a, b) => uses_it(c) || uses_it(a) || uses_it(b),
        ExprKind::Match(x, arms) => {
            uses_it(x) || arms.iter().any(|(p, v)| pat_uses_it(p) || uses_it(v))
        }
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program> {
    let mut p = Parser { t: tokens, i: 0, pending_gt: false };
    let mut body = Vec::new();
    while !p.check(&Tok::Eof) {
        if p.eat(&Tok::Newline) {
            continue;
        }
        if p.check(&Tok::Import) {
            if body.iter().any(|s: &Stmt| !matches!(s.kind, StmtKind::Import { .. })) {
                return Err(Error::parse("imports go at the top of the file, before anything else", p.span()));
            }
            body.push(p.import()?);
            continue;
        }
        if matches!(p.peek(), Tok::Struct | Tok::Enum) {
            let start = p.span();
            let decl = p.type_decl()?;
            body.push(Stmt { kind: StmtKind::TypeDecl(Rc::new(decl)), span: start.to(p.prev_span()) });
            continue;
        }
        body.push(p.stmt()?);
    }
    check_unused(&body, true)?;
    Ok(Program { body })
}

/// A comparison, `in` or logic expression whose value is thrown away is
/// almost always a missing `assert` (or `return`), so it's an error. Calls
/// stay allowed: `seen.add(p)` is used for its effect. `gives_value` says
/// whether the block's last statement is its value (a function body, the
/// script, a `match` arm in such a place).
fn check_unused(stmts: &[Stmt], gives_value: bool) -> Result<()> {
    let value_stmt = if gives_value {
        stmts.iter().rposition(|s| !matches!(s.kind, StmtKind::Test { .. } | StmtKind::TestBlock { .. } | StmtKind::Fun(_) | StmtKind::TypeDecl(_) | StmtKind::Import { .. }))
    } else {
        None
    };
    for (i, s) in stmts.iter().enumerate() {
        let used = value_stmt == Some(i);
        match &s.kind {
            StmtKind::Expr(e) if !used && is_bool_op(e) => {
                return Err(Error::parse("this comparison's result is never used; did you mean `assert …`?", e.span));
            }
            StmtKind::If { then, els, .. } => {
                check_unused(then, false)?;
                if let Some(els) = els {
                    check_unused(els, false)?;
                }
            }
            StmtKind::While { body, .. } | StmtKind::For { body, .. } | StmtKind::TestBlock { body, .. } => check_unused(body, false)?,
            StmtKind::Match { arms, .. } => {
                for (_, arm) in arms {
                    check_unused(arm, used)?;
                }
            }
            StmtKind::Fun(f) => check_unused(&f.body, true)?,
            _ => {}
        }
    }
    Ok(())
}

fn is_bool_op(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Binary(op, ..) => matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge),
        ExprKind::And(..) | ExprKind::Or(..) | ExprKind::In { .. } | ExprKind::Unary(UnOp::Not, _) => true,
        _ => false,
    }
}

struct Parser<'a> {
    t: &'a [Token],
    i: usize,
    /// A `>>` closed two type parameter lists (`Map<int, Set<int>>`); the second `>` is still pending.
    pending_gt: bool,
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

    /// `import a, B from "path.egg"`. `from` is only special here.
    fn import(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let mut names = Vec::new();
        loop {
            let span = self.span();
            names.push((self.ident("a name to import")?, span));
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        match self.peek() {
            Tok::Ident(n) if &**n == "from" => {
                self.advance();
            }
            other => return Err(Error::parse(format!("expected `from \"file.egg\"`, found {}", describe(other)), self.span())),
        }
        let path_span = self.span();
        let path = match self.peek().clone() {
            Tok::Str(s) => {
                self.advance();
                s
            }
            other => return Err(Error::parse(format!("expected the file to import from, as a string, found {}", describe(&other)), path_span)),
        };
        self.end_stmt()?;
        Ok(Stmt { kind: StmtKind::Import { names, path, path_span }, span: start.to(path_span) })
    }

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
            Tok::If if !self.if_is_expr() => return self.if_stmt(),
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
            Tok::Struct | Tok::Enum => return Err(Error::parse("declare structs and enums at the top level, not inside a block", start)),
            Tok::Import => return Err(Error::parse("imports go at the top of the file, not inside a block", start)),
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
                    Tok::AmpEq => Some(Some(BinOp::BitAnd)),
                    Tok::PipeEq => Some(Some(BinOp::BitOr)),
                    Tok::CaretEq => Some(Some(BinOp::BitXor)),
                    Tok::ShlEq => Some(Some(BinOp::Shl)),
                    Tok::ShrEq => Some(Some(BinOp::Shr)),
                    _ => None,
                };
                if let Some(op) = op {
                    self.advance();
                    if !matches!(target.kind, ExprKind::Var(_) | ExprKind::Index { .. } | ExprKind::Field { .. }) {
                        return Err(Error::parse("can only assign to a variable, an indexed element or a struct field", target.span));
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
        if self.peek() == &Tok::LParen {
            // `(p, q)` is a tuple of patterns; `(p)` is just `p`.
            let start = self.span();
            self.advance();
            let mut elems = Vec::new();
            let mut comma = false;
            while !self.eat(&Tok::RParen) {
                elems.push(self.match_pat()?);
                if self.eat(&Tok::Comma) {
                    comma = true;
                } else {
                    self.expect(&Tok::RParen, "`,` or `)`")?;
                    break;
                }
            }
            if elems.len() == 1 && !comma {
                return Ok(elems.pop().unwrap());
            }
            return Ok(MatchPat::Tuple(elems, start.to(self.prev_span())));
        }
        if let Tok::Ident(n) = self.peek().clone() {
            if &*n == "_" {
                self.advance();
                return Ok(MatchPat::Wild);
            }
            if self.peek_at(1) == &Tok::LParen && is_pascal(&n) {
                let start = self.span();
                self.advance();
                self.advance();
                let mut binds = Vec::new();
                while !self.eat(&Tok::RParen) {
                    binds.push(self.pattern()?);
                    if !self.eat(&Tok::Comma) {
                        self.expect(&Tok::RParen, "`,` or `)`")?;
                        break;
                    }
                }
                return Ok(MatchPat::Variant { name: n, binds, span: start.to(self.prev_span()) });
            }
        }
        Ok(MatchPat::Value(self.or()?))
    }

    /// `struct Name:` with one `field: type` per line, or `enum Name:` with
    /// one `Variant` / `Variant(field: type, …)` per line.
    fn type_decl(&mut self) -> Result<TypeDecl> {
        let start = self.span();
        let is_struct = self.advance().tok == Tok::Struct;
        let kind = if is_struct { "struct" } else { "enum" };
        let name = self.pascal_name(&format!("a {kind} name"))?;
        self.expect(&Tok::Colon, "`:`")?;
        self.expect(&Tok::Newline, &format!("a new line after `{kind} {name}:`"))?;
        self.expect(&Tok::Indent, if is_struct { "indented fields (`name: type`)" } else { "indented variants" })?;
        let mut fields = Vec::new();
        let mut variants: Vec<VariantDecl> = Vec::new();
        while !self.eat(&Tok::Dedent) {
            if self.eat(&Tok::Newline) {
                continue;
            }
            if is_struct {
                fields.push(self.field(&fields)?);
            } else {
                let vspan = self.span();
                let vname = self.pascal_name("a variant name")?;
                if variants.iter().any(|v| v.name == vname) {
                    return Err(Error::parse(format!("variant `{vname}` is declared twice"), vspan));
                }
                let mut vfields = Vec::new();
                if self.eat(&Tok::LParen) {
                    while !self.eat(&Tok::RParen) {
                        vfields.push(self.field(&vfields)?);
                        if !self.eat(&Tok::Comma) {
                            self.expect(&Tok::RParen, "`,` or `)`")?;
                            break;
                        }
                    }
                    if vfields.is_empty() {
                        return Err(Error::parse(format!("a variant without fields is written without parentheses: `{vname}`"), vspan));
                    }
                }
                variants.push(VariantDecl { name: vname, fields: vfields, span: vspan.to(self.prev_span()) });
            }
            if !self.check(&Tok::Dedent) {
                self.expect(&Tok::Newline, "a new line")?;
            }
        }
        if is_struct {
            if fields.is_empty() {
                return Err(Error::parse(format!("struct `{name}` has no fields"), start));
            }
            variants.push(VariantDecl { name: name.clone(), fields, span: start });
        }
        if variants.is_empty() {
            return Err(Error::parse(format!("enum `{name}` has no variants"), start));
        }
        Ok(TypeDecl { name, is_struct, variants, span: start })
    }

    /// `name: type` or `name: type = default`, in a struct or a variant.
    fn field(&mut self, earlier: &[Param]) -> Result<Param> {
        let span = self.span();
        let name = self.ident("a field name")?;
        if &*name == "it" {
            return Err(Error::parse(IT_RESERVED, span));
        }
        if earlier.iter().any(|p| p.name == name) {
            return Err(Error::parse(format!("field `{name}` is declared twice"), span));
        }
        if !self.eat(&Tok::Colon) {
            return Err(Error::parse(format!("field `{name}` needs a type (`{name}: int`)"), span));
        }
        let ty = self.ty()?;
        let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
        if default.is_none() && earlier.iter().any(|p| p.default.is_some()) {
            return Err(Error::parse("fields with defaults must come after those without", span));
        }
        Ok(Param { name, ty, inout: false, default })
    }

    fn pascal_name(&mut self, what: &str) -> Result<Name> {
        let span = self.span();
        let name = self.ident(what)?;
        if !is_pascal(&name) {
            return Err(Error::parse(format!("struct, enum and variant names use PascalCase: `{name}`"), span));
        }
        Ok(name)
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
            if &*pname == "it" {
                return Err(Error::parse(IT_RESERVED, pspan));
            }
            if !self.eat(&Tok::Colon) {
                return Err(Error::parse(format!("parameter `{pname}` needs a type (`{pname}: int`)"), pspan));
            }
            let ty = self.ty()?;
            let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            if default.is_some() {
                seen_default = true;
            } else if seen_default {
                return Err(Error::parse("parameters with defaults must come after those without", pspan));
            }
            params.push(Param { name: pname, ty, inout, default });
            if !self.eat(&Tok::Comma) {
                self.expect(&Tok::RParen, "`,` or `)`")?;
                break;
            }
        }
        let ret = if self.eat(&Tok::Arrow) { Some(self.ty()?) } else { None };
        let body = self.block()?;
        Ok(FunDecl { name, params, ret, memo, body, span: start })
    }

    fn ty(&mut self) -> Result<Type> {
        if self.eat(&Tok::LBracket) {
            let elem = self.ty()?;
            self.expect(&Tok::RBracket, "`]` in a list type")?;
            return Ok(Type::List(Box::new(elem)));
        }
        if self.eat(&Tok::LParen) {
            let mut elems = Vec::new();
            while !self.eat(&Tok::RParen) {
                elems.push(self.ty()?);
                if !self.eat(&Tok::Comma) {
                    self.expect(&Tok::RParen, "`)` in a tuple type")?;
                    break;
                }
            }
            return Ok(Type::Tuple(elems));
        }
        let span = self.span();
        let name = self.ident("a type")?;
        let mut args = Vec::new();
        if self.eat(&Tok::Lt) {
            loop {
                args.push(self.ty()?);
                if self.eat_gt() {
                    break;
                }
                self.expect(&Tok::Comma, "`,` or `>` in a type")?;
            }
        }
        let (ty, arity) = match &*name {
            "int" => (Type::Int, 0),
            "bool" => (Type::Bool, 0),
            "char" => (Type::Char, 0),
            "string" => (Type::Str, 0),
            "float" => (Type::Float, 0),
            "Set" => (Type::Container("Set", Vec::new()), 1),
            "Map" => (Type::Container("Map", Vec::new()), 2),
            "Queue" => (Type::Container("Queue", Vec::new()), 1),
            "Stack" => (Type::Container("Stack", Vec::new()), 1),
            "MinQueue" => (Type::Container("MinQueue", Vec::new()), 1),
            "Grid" => (Type::Container("Grid", Vec::new()), 1),
            _ if is_pascal(&name) && args.is_empty() => return Ok(Type::Named(name)),
            _ => {
                let hint = if &*name == "str" { "; did you mean `string`?" } else { "" };
                return Err(Error::parse(format!("unknown type `{name}`{hint}"), span));
            }
        };
        if args.len() != arity {
            let want = match arity {
                0 => "no type parameters".to_string(),
                1 => "one type parameter".to_string(),
                n => format!("{n} type parameters"),
            };
            return Err(Error::parse(format!("`{name}` takes {want}"), span));
        }
        Ok(match ty {
            Type::Container(n, _) => Type::Container(n, args),
            t => t,
        })
    }

    /// The `>` that closes a type parameter list, splitting a `>>` in two.
    fn eat_gt(&mut self) -> bool {
        if self.pending_gt {
            self.pending_gt = false;
            return true;
        }
        if self.eat(&Tok::Shr) {
            self.pending_gt = true;
            return true;
        }
        self.eat(&Tok::Gt)
    }

    fn pattern(&mut self) -> Result<Pattern> {
        match self.peek().clone() {
            Tok::Ident(n) => {
                if &*n == "it" {
                    return Err(Error::parse(IT_RESERVED, self.span()));
                }
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
            Tok::LBracket if self.parens_then_arrow() => {
                let p = self.pattern()?;
                self.expect(&Tok::FatArrow, "`=>`")?;
                let body = self.expr()?;
                let span = start.to(body.span);
                return Ok(Expr { kind: ExprKind::Lambda(Rc::new(Lambda { params: vec![p], body })), span });
            }
            Tok::LParen if self.parens_then_arrow() => {
                self.advance();
                let mut params = Vec::new();
                while !self.eat(&Tok::RParen) {
                    params.push(self.pattern()?);
                    // Lambda parameter types are optional and not checked.
                    if self.eat(&Tok::Colon) {
                        self.ty()?;
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

    /// At `if`: is this an if-expression (`then` before the line's `:`)?
    fn if_is_expr(&self) -> bool {
        let mut depth = 0;
        for t in &self.t[self.i..] {
            match t.tok {
                Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RParen | Tok::RBracket => depth -= 1,
                Tok::Then => return true,
                Tok::Colon if depth == 0 => return false,
                Tok::Newline | Tok::Eof => return false,
                _ => {}
            }
        }
        false
    }

    /// At `(` or `[`: is this a lambda's parameters (followed by `=>`)?
    fn parens_then_arrow(&self) -> bool {
        let mut depth = 0;
        let mut j = self.i;
        while j < self.t.len() {
            match self.t[j].tok {
                Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RParen | Tok::RBracket => {
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
        let start = if matches!(self.peek(), Tok::DotDot | Tok::DotDotEq) { None } else { Some(self.bit_or()?) };
        let inclusive = match self.peek() {
            Tok::DotDot => false,
            Tok::DotDotEq => true,
            _ => return Ok(start.unwrap()),
        };
        self.advance();
        let end = if self.starts_expr() { Some(Box::new(self.bit_or()?)) } else { None };
        if inclusive && end.is_none() {
            return Err(Error::parse("`..=` needs an end", self.span()));
        }
        let span = start_span.to(self.prev_span());
        Ok(Expr { kind: ExprKind::Range { start: start.map(Box::new), end, inclusive }, span })
    }

    /// Bitwise operators, as in Rust: tighter than comparisons, so
    /// `n & 1 == 0` is `(n & 1) == 0`. From loosest: `|`, `^`, `&`, `<<` `>>`.
    fn bit_or(&mut self) -> Result<Expr> {
        let mut l = self.bit_xor()?;
        while self.eat(&Tok::Pipe) {
            let r = self.bit_xor()?;
            l = Self::binary(l, BinOp::BitOr, r);
        }
        Ok(l)
    }

    fn bit_xor(&mut self) -> Result<Expr> {
        let mut l = self.bit_and()?;
        while self.eat(&Tok::Caret) {
            let r = self.bit_and()?;
            l = Self::binary(l, BinOp::BitXor, r);
        }
        Ok(l)
    }

    fn bit_and(&mut self) -> Result<Expr> {
        let mut l = self.shift()?;
        while self.eat(&Tok::Amp) {
            let r = self.shift()?;
            l = Self::binary(l, BinOp::BitAnd, r);
        }
        Ok(l)
    }

    fn shift(&mut self) -> Result<Expr> {
        let mut l = self.add()?;
        loop {
            let op = match self.peek() {
                Tok::Shl => BinOp::Shl,
                Tok::Shr => BinOp::Shr,
                _ => return Ok(l),
            };
            self.advance();
            let r = self.add()?;
            l = Self::binary(l, op, r);
        }
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
        let mut l = self.cast()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => return Ok(l),
            };
            self.advance();
            let r = self.cast()?;
            l = Self::binary(l, op, r);
        }
    }

    /// `x as int`: tighter than `*`, looser than unary `-`, as in Rust.
    fn cast(&mut self) -> Result<Expr> {
        let mut e = self.unary()?;
        while self.eat(&Tok::As) {
            let ty = self.ty()?;
            let span = e.span.to(self.prev_span());
            e = Expr { kind: ExprKind::Cast(Box::new(e), ty), span };
        }
        Ok(e)
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
                let name = self.ident("a method or field name")?;
                if !self.eat(&Tok::LParen) {
                    let span = e.span.to(self.prev_span());
                    e = Expr { kind: ExprKind::Field { recv: Box::new(e), name }, span };
                    continue;
                }
                let mut args = self.args()?;
                if LAMBDA_METHODS.contains(&&*name) {
                    for a in &mut args {
                        wrap_it(&mut a.value);
                    }
                }
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

/// Struct, enum and variant names start with an uppercase letter.
fn is_pascal(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}
