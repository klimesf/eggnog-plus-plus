//! The tree-walking evaluator: statements, expressions, places, calls and
//! lazy chains.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use crate::types;
use crate::ast::*;
use crate::builtins::{self, Args};
use crate::error::{self, Error, Result, Span};
use crate::value::*;

pub enum Flow {
    /// Finished normally; carries the value of the last statement if it was an expression.
    Normal(Option<Value>),
    Break,
    Continue,
    Return(Value),
}

/// An assignable location: a variable followed by zero or more indexes and fields.
pub struct Place {
    root: Name,
    root_span: Span,
    keys: Vec<(Key, Span)>,
}

enum Key {
    Index(Value),
    Field(Name),
}

/// Names a struct or variant can't take: built-in types and constants.
const RESERVED_TYPE_NAMES: &[&str] = &["Set", "Map", "Queue", "Stack", "MinQueue", "Grid"];

pub struct Interp {
    pub env: Env,
    pub base_dir: PathBuf,
    /// Names of the declared structs and enums.
    type_names: HashSet<Name>,
    /// Names brought in by `import`; a local `fun` can't reuse one.
    imported: HashSet<Name>,
}

/// A parsed library file, shared by every import of it, so its types are the
/// same `Rc`s in the importer and in the library's own functions.
struct Library {
    program: Program,
    dir: PathBuf,
    /// The path for messages, relative to where the runner started.
    name: String,
}

thread_local! {
    static LIBRARIES: std::cell::RefCell<HashMap<PathBuf, Rc<Library>>> = Default::default();
}

/// `a/b/../c.egg` → `a/c.egg`, for readable file names in messages.
fn tidy(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir if matches!(out.components().next_back(), Some(Component::Normal(_))) => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

fn load_library(base_dir: &Path, path: &str, span: Span) -> Result<Rc<Library>> {
    let full = tidy(&base_dir.join(path));
    let key = full.canonicalize().unwrap_or_else(|_| full.clone());
    if let Some(lib) = LIBRARIES.with(|l| l.borrow().get(&key).cloned()) {
        return Ok(lib);
    }
    let name = full.display().to_string();
    let source = std::fs::read_to_string(&full)
        .map_err(|e| Error::runtime(format!("can't read `{name}`: {e}"), span))?;
    let base = error::register_source(name.clone(), source.as_str().into());
    let program = crate::parser::parse(&crate::lexer::tokenize_at(&source, base)?)?;
    for s in &program.body {
        match &s.kind {
            StmtKind::Fun(_) | StmtKind::TypeDecl(_) | StmtKind::Test { .. } | StmtKind::TestBlock { .. } => {}
            StmtKind::Import { .. } => {
                return Err(Error::runtime("a library file can't import; only the solution files import", s.span));
            }
            _ => {
                return Err(Error::runtime(
                    format!("`{name}` can't be imported: a library file holds only `fun`, `struct`, `enum` and `test` at its top level"),
                    s.span,
                ));
            }
        }
    }
    let dir = full.parent().map(Path::to_path_buf).unwrap_or_default();
    let lib = Rc::new(Library { program, dir, name });
    LIBRARIES.with(|l| l.borrow_mut().insert(key, lib.clone()));
    Ok(lib)
}

impl Interp {
    /// A fresh interpreter whose script scope defines `input`.
    pub fn new(input: &str, base_dir: PathBuf) -> Self {
        let globals = builtins::globals();
        let env = new_boundary(Some(globals));
        env.borrow_mut().vars.insert("input".into(), Value::Str(input.into()));
        Interp { env, base_dir, type_names: Default::default(), imported: Default::default() }
    }

    pub fn in_env<R>(&mut self, env: Env, f: impl FnOnce(&mut Self) -> Result<R>) -> Result<R> {
        let saved = std::mem::replace(&mut self.env, env);
        let r = f(self);
        self.env = saved;
        r
    }

    /// Like `in_env`, for a scope made just for this block or call. A local
    /// `fun` holds its defining scope, which holds the `fun`: a cycle that `Rc`
    /// never frees. Functions can't escape, so drop them when the scope ends.
    pub fn in_scope<R>(&mut self, scope: Env, f: impl FnOnce(&mut Self) -> Result<R>) -> Result<R> {
        let r = self.in_env(scope.clone(), f);
        scope.borrow_mut().vars.retain(|_, v| !matches!(v, Value::Func(_)));
        r
    }

    /// Runs a whole program and returns its answer.
    pub fn run(&mut self, program: &Program) -> Result<Value> {
        self.declare_imports(program)?;
        self.declare_types(program)?;
        let answer = match self.exec_stmts(&program.body)? {
            Flow::Return(v) | Flow::Normal(Some(v)) => v,
            Flow::Normal(None) => {
                let span = program.body.iter().rev().find(|s| !is_test(s)).map_or(Span::default(), |s| s.span);
                let hint = if ends_with_block_if(&program.body) { BLOCK_IF_HINT } else { "" };
                return Err(Error::runtime(
                    format!("the program has no answer: its last statement must be an expression (or use `return`){hint}"),
                    span,
                ));
            }
            Flow::Break | Flow::Continue => return Err(Error::runtime("`break`/`continue` outside a loop", Span::default())),
        };
        if let Value::Chain(c) = &answer {
            return Err(Error::runtime("the answer is an unfinished chain; end it with .sum(), .count(), .to_list(), …", c.span));
        }
        if let Value::Unit = answer {
            let span = program.body.iter().rev().find(|s| !is_test(s)).map_or(Span::default(), |s| s.span);
            return Err(Error::runtime("the program's last expression produced no value", span));
        }
        Ok(answer)
    }

    /// Brings the names listed by each `import` into the script scope. A
    /// library's functions run in a scope of their own that holds the whole
    /// library, so they can call helpers that weren't imported.
    pub fn declare_imports(&mut self, program: &Program) -> Result<()> {
        for s in &program.body {
            let StmtKind::Import { names, path, path_span } = &s.kind else { continue };
            let lib = load_library(&self.base_dir, path, *path_span)?;
            let mut li = Interp::new("", lib.dir.clone());
            li.env.borrow_mut().vars.remove("input");
            li.declare_types(&lib.program)?;
            for f in lib.program.body.iter().filter(|s| matches!(s.kind, StmtKind::Fun(_))) {
                li.exec_stmts(std::slice::from_ref(f))?;
            }
            for (name, span) in names {
                let ty = lib.program.body.iter().find_map(|s| match &s.kind {
                    StmtKind::TypeDecl(d) if &d.name == name => Some(d.clone()),
                    _ => None,
                });
                let mut bring: Vec<Name> = Vec::new();
                if let Some(decl) = &ty {
                    bring.extend(decl.variants.iter().map(|v| v.name.clone()));
                } else if matches!(li.env.borrow().vars.get(name), Some(Value::Func(_))) {
                    bring.push(name.clone());
                } else {
                    let hint = if lib.program.body.iter().any(|s| match &s.kind {
                        StmtKind::TypeDecl(d) => d.variants.iter().any(|v| &v.name == name),
                        _ => false,
                    }) {
                        "; import its enum to get all of its variants"
                    } else {
                        ""
                    };
                    return Err(Error::runtime(format!("`{}` declares no function, struct or enum `{name}`{hint}", lib.name), *span));
                }
                for n in bring.iter().chain(ty.as_ref().map(|d| &d.name)) {
                    if RESERVED_TYPE_NAMES.contains(&&**n) || self.lookup(n).is_some() || self.type_names.contains(n) {
                        return Err(Error::runtime(format!("`{n}` is already defined; it can't be imported too"), *span));
                    }
                }
                for n in bring {
                    let v = li.env.borrow().vars.get(&n).cloned().expect("declared in the library");
                    self.env.borrow_mut().vars.insert(n.clone(), v);
                    self.imported.insert(n);
                }
                if let Some(decl) = ty {
                    self.type_names.insert(decl.name.clone());
                    self.imported.insert(decl.name.clone());
                }
            }
        }
        Ok(())
    }

    /// Types are known everywhere in the file, like in a compiled language.
    pub fn declare_types(&mut self, program: &Program) -> Result<()> {
        for s in &program.body {
            if let StmtKind::TypeDecl(decl) = &s.kind {
                self.declare_type(decl)?;
            }
        }
        Ok(())
    }

    /// Defines a struct's constructor, or each variant of an enum.
    fn declare_type(&mut self, decl: &Rc<TypeDecl>) -> Result<()> {
        let mut names: Vec<(&Name, Span)> = decl.variants.iter().map(|v| (&v.name, v.span)).collect();
        if !decl.is_struct {
            names.push((&decl.name, decl.span));
        }
        for (name, span) in names {
            if RESERVED_TYPE_NAMES.contains(&&**name) {
                return Err(Error::runtime(format!("`{name}` is a built-in type; pick another name"), span));
            }
            if let Some(v) = self.lookup(name) {
                let what = match v {
                    Value::Record(..) | Value::Ctor(_) => "a struct or variant",
                    Value::Func(_) => "a function",
                    _ => "a built-in constant",
                };
                return Err(Error::runtime(format!("`{name}` is already {what}; pick another name"), span));
            }
            if self.type_names.contains(name) {
                return Err(Error::runtime(format!("`{name}` is already a type; pick another name"), span));
            }
        }
        if !self.type_names.insert(decl.name.clone()) {
            return Err(Error::runtime(format!("`{}` is declared twice", decl.name), decl.span));
        }
        for (index, v) in decl.variants.iter().enumerate() {
            let def = Rc::new(VariantDef { decl: decl.clone(), index });
            let value = if v.fields.is_empty() { Value::Record(def, Rc::new(Vec::new())) } else { Value::Ctor(def) };
            self.env.borrow_mut().vars.insert(v.name.clone(), value);
        }
        Ok(())
    }

    /// `Monkey(items, "old * 19", divisor = 23)`: arguments bind to fields like
    /// to a function's parameters.
    fn construct(&mut self, def: &Rc<VariantDef>, args: &[Arg], span: Span) -> Result<Value> {
        let v = def.variant();
        let mut values: Vec<Option<Value>> = vec![None; v.fields.len()];
        for (k, arg) in args.iter().enumerate() {
            let idx = match &arg.name {
                Some(n) => v
                    .fields
                    .iter()
                    .position(|p| &p.name == n)
                    .ok_or_else(|| Error::runtime(format!("`{}` has no field named `{n}`", v.name), arg.value.span))?,
                None if k < v.fields.len() => k,
                None => return Err(Error::runtime(format!("too many fields for `{}`", v.name), arg.value.span)),
            };
            if values[idx].is_some() {
                return Err(Error::runtime(format!("field `{}` given twice", v.fields[idx].name), arg.value.span));
            }
            let x = self.eval(&arg.value)?;
            if let Value::Chain(_) = x {
                return Err(Error::runtime("a chain can't be stored in a field; use .to_list() first", arg.value.span));
            }
            values[idx] = Some(x);
        }
        let mut fields = Vec::with_capacity(values.len());
        for (p, x) in v.fields.iter().zip(values) {
            let x = match (x, &p.default) {
                (Some(x), _) => x,
                (None, Some(d)) => self.eval(d)?,
                (None, None) => return Err(Error::runtime(format!("missing field `{}` for `{}`", p.name, v.name), span)),
            };
            if !types::fits(&x, &p.ty) {
                return Err(Error::runtime(format!("`{}` expects `{}: {}`, got {}", v.name, p.name, p.ty, types::describe_typed(&x)), span));
            }
            fields.push(x);
        }
        Ok(Value::Record(def.clone(), Rc::new(fields)))
    }

    // ---- variables ---------------------------------------------------------

    pub fn lookup(&self, name: &str) -> Option<Value> {
        let mut env = Some(self.env.clone());
        while let Some(e) = env {
            if let Some(v) = e.borrow().vars.get(name) {
                return Some(v.clone());
            }
            env = e.borrow().parent.clone();
        }
        None
    }

    fn find_scope(&self, name: &str) -> Option<Env> {
        let mut env = Some(self.env.clone());
        while let Some(e) = env {
            if e.borrow().vars.contains_key(name) {
                return Some(e);
            }
            env = e.borrow().parent.clone();
        }
        None
    }

    /// A `let` in an inner block may not reuse a name from an enclosing block
    /// of the same script, function or lambda body.
    fn check_outer_names(&self, pat: &Pattern, span: Span) -> Result<()> {
        match pat {
            Pattern::Wild => Ok(()),
            Pattern::Name(n) => {
                if self.env.borrow().boundary {
                    return Ok(());
                }
                let mut env = self.env.borrow().parent.clone();
                while let Some(e) = env {
                    if e.borrow().vars.contains_key(n) {
                        return Err(Error::runtime(format!("`let {n}` hides the `{n}` of an enclosing block; to change it, drop the `let`"), span));
                    }
                    if e.borrow().boundary {
                        break;
                    }
                    env = e.borrow().parent.clone();
                }
                Ok(())
            }
            Pattern::Tuple(ps) | Pattern::List(ps) => ps.iter().try_for_each(|p| self.check_outer_names(p, span)),
        }
    }

    fn define(&mut self, name: &Name, v: Value, span: Span) -> Result<()> {
        if let Value::Chain(_) = v {
            return Err(Error::runtime("a chain can't be stored in a variable; consume it here (.to_list(), .sum(), a for loop, …)", span));
        }
        if let Value::Ctor(def) = &v {
            return Err(Error::runtime(format!("`{}` needs its fields: `{}(…)`", def.name(), def.name()), span));
        }
        self.env.borrow_mut().vars.insert(name.clone(), v);
        Ok(())
    }

    pub fn bind_pattern(&mut self, pat: &Pattern, v: Value, span: Span) -> Result<()> {
        match pat {
            Pattern::Wild => Ok(()),
            Pattern::Name(n) => self.define(n, v, span),
            Pattern::Tuple(pats) => match &v {
                Value::Tuple(items) if items.len() == pats.len() => {
                    for (p, item) in pats.iter().zip(items.iter()) {
                        self.bind_pattern(p, item.clone(), span)?;
                    }
                    Ok(())
                }
                _ => Err(Error::runtime(format!("can't destructure {} into a tuple of {}", describe(&v), pats.len()), span)),
            },
            Pattern::List(pats) => match &v {
                Value::List(items) if items.len() == pats.len() => {
                    for (p, item) in pats.iter().zip(items.iter()) {
                        self.bind_pattern(p, item.clone(), span)?;
                    }
                    Ok(())
                }
                Value::List(items) => {
                    Err(Error::runtime(format!("expected a list of {} elements, got {}: {}", pats.len(), items.len(), v.repr()), span))
                }
                _ => Err(Error::runtime(format!("can't destructure {} as a list", describe(&v)), span)),
            },
        }
    }

    // ---- places ------------------------------------------------------------

    fn place(&mut self, e: &Expr) -> Result<Option<Place>> {
        match &e.kind {
            ExprKind::Var(n) => Ok(Some(Place { root: n.clone(), root_span: e.span, keys: Vec::new() })),
            ExprKind::Index { recv, index } => {
                let Some(mut p) = self.place(recv)? else { return Ok(None) };
                let k = self.eval(index)?;
                p.keys.push((Key::Index(k), index.span));
                Ok(Some(p))
            }
            ExprKind::Field { recv, name } => {
                let Some(mut p) = self.place(recv)? else { return Ok(None) };
                p.keys.push((Key::Field(name.clone()), e.span));
                Ok(Some(p))
            }
            _ => Ok(None),
        }
    }

    /// Runs `f` on the value stored at `place`. `create` lets the last key
    /// be inserted into a map (for assignment).
    fn with_place<R>(&mut self, place: &Place, create: bool, f: impl FnOnce(&mut Value) -> Result<R>) -> Result<R> {
        let env = self
            .find_scope(&place.root)
            .ok_or_else(|| Error::runtime(format!("unknown variable `{}` (declare it with `let`)", place.root), place.root_span))?;
        let mut scope = env.borrow_mut();
        if scope.readonly.contains(&place.root) {
            return Err(Error::runtime(
                format!("parameter `{}` is read-only; mark it `inout` to change the caller's value, or copy it with `let`", place.root),
                place.root_span,
            ));
        }
        if scope.bound.contains(&place.root) {
            return Err(Error::runtime(
                format!("`{}` is a copy of a field from `match`; to change the value, assign a whole new one", place.root),
                place.root_span,
            ));
        }
        let mut slot = scope.vars.get_mut(&place.root).unwrap();
        let last = place.keys.len().saturating_sub(1);
        for (n, (k, span)) in place.keys.iter().enumerate() {
            slot = match k {
                Key::Index(k) => builtins::index_mut(slot, k, *span, create && n == last)?,
                Key::Field(name) => field_mut(slot, name, *span)?,
            };
        }
        f(slot)
    }

    // ---- statements --------------------------------------------------------

    pub fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<Flow> {
        let mut last = None;
        for s in stmts {
            match self.exec_stmt(s)? {
                // A block `if` is a statement, so it never gives the block its value
                // (the expression form is `if c then a else b`).
                Flow::Normal(v) => {
                    if !is_test(s) {
                        last = if matches!(s.kind, StmtKind::If { .. }) { None } else { v };
                    }
                }
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal(last))
    }

    fn exec_block(&mut self, stmts: &[Stmt]) -> Result<Flow> {
        let scope = new_scope(Some(self.env.clone()));
        self.in_scope(scope, |s| s.exec_stmts(stmts))
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow> {
        match &stmt.kind {
            StmtKind::Let(pat, value) => {
                self.check_outer_names(pat, stmt.span)?;
                let v = self.eval(value)?;
                if let Pattern::Name(n) = pat {
                    self.env.borrow_mut().readonly.remove(n);
                    self.env.borrow_mut().bound.remove(n);
                }
                self.bind_pattern(pat, v, stmt.span)?;
                Ok(Flow::Normal(None))
            }
            StmtKind::Assign { target, op, value } => {
                let v = self.eval(value)?;
                if let Value::Chain(_) = v {
                    return Err(Error::runtime("a chain can't be stored; consume it here (.to_list(), .sum(), …)", value.span));
                }
                let new = match op {
                    Some(op) => {
                        let cur = self.eval(target)?;
                        builtins::binary(*op, cur, v, stmt.span)?
                    }
                    None => v,
                };
                let place = self
                    .place(target)?
                    .ok_or_else(|| Error::runtime("can only assign to a variable or an element of one", target.span))?;
                self.with_place(&place, true, |slot| {
                    *slot = new;
                    Ok(())
                })?;
                Ok(Flow::Normal(None))
            }
            StmtKind::Expr(e) => Ok(Flow::Normal(Some(self.eval(e)?))),
            StmtKind::If { cond, then, els } => {
                let taken = match cond {
                    Cond::Expr(e) => {
                        if self.truthy(e)? {
                            return self.exec_block(then);
                        }
                        false
                    }
                    Cond::Bind(pat, e) => {
                        let v = self.eval_raw(e)?;
                        if let Value::Missing = v {
                            false
                        } else {
                            let scope = new_scope(Some(self.env.clone()));
                            return self.in_scope(scope, |s| {
                                s.bind_pattern(pat, v, e.span)?;
                                s.exec_stmts(then)
                            });
                        }
                    }
                };
                match (taken, els) {
                    (false, Some(els)) => self.exec_block(els),
                    _ => Ok(Flow::Normal(None)),
                }
            }
            StmtKind::While { cond, body } => {
                loop {
                    let scope = new_scope(Some(self.env.clone()));
                    let flow = self.in_scope(scope, |s| {
                        let go = match cond {
                            Cond::Expr(e) => s.truthy(e)?,
                            Cond::Bind(pat, e) => {
                                let v = s.eval_raw(e)?;
                                if let Value::Missing = v {
                                    false
                                } else {
                                    s.bind_pattern(pat, v, e.span)?;
                                    true
                                }
                            }
                        };
                        if go { Ok(Some(s.exec_stmts(body)?)) } else { Ok(None) }
                    })?;
                    match flow {
                        None | Some(Flow::Break) => break,
                        Some(Flow::Normal(_)) | Some(Flow::Continue) => {}
                        Some(Flow::Return(v)) => return Ok(Flow::Return(v)),
                    }
                }
                Ok(Flow::Normal(None))
            }
            StmtKind::For { pat, iter, body } => {
                let it = self.eval(iter)?;
                let chain = builtins::to_chain(&it, iter.span)?;
                let mut result = Flow::Normal(None);
                let span = iter.span;
                self.drive(&chain, &mut |s, v| {
                    let scope = new_scope(Some(s.env.clone()));
                    let f = s.in_scope(scope, |s| {
                        s.bind_pattern(pat, v, span)?;
                        s.exec_stmts(body)
                    })?;
                    match f {
                        Flow::Normal(_) | Flow::Continue => Ok(true),
                        Flow::Break => Ok(false),
                        Flow::Return(v) => {
                            result = Flow::Return(v);
                            Ok(false)
                        }
                    }
                })?;
                Ok(result)
            }
            StmtKind::Match { subject, arms } => {
                let v = self.eval(subject)?;
                self.check_exhaustive(arms.iter().map(|(p, _)| p), &v, subject.span)?;
                for (pat, body) in arms {
                    if let Some(binds) = self.match_pat(pat, &v)? {
                        let scope = new_scope(Some(self.env.clone()));
                        return self.in_scope(scope, |s| {
                            s.bind_fields(binds)?;
                            s.exec_stmts(body)
                        });
                    }
                }
                Err(Error::runtime(format!("no match arm for {}", v.repr()), subject.span))
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::Continue),
            StmtKind::Return(e) => Ok(Flow::Return(match e {
                Some(e) => self.eval(e)?,
                None => Value::Unit,
            })),
            StmtKind::TypeDecl(_) => Ok(Flow::Normal(None)),
            StmtKind::Fun(decl) => {
                if self.imported.contains(&decl.name) {
                    return Err(Error::runtime(format!("`{}` is imported; pick another name", decl.name), decl.span));
                }
                if let Some(Value::Record(..) | Value::Ctor(_)) = self.lookup(&decl.name) {
                    return Err(Error::runtime(format!("`{}` is already a struct or variant; pick another name", decl.name), decl.span));
                }
                let f = Func { decl: decl.clone(), env: self.env.clone(), cache: Default::default() };
                self.env.borrow_mut().vars.insert(decl.name.clone(), Value::Func(Rc::new(f)));
                Ok(Flow::Normal(None))
            }
            StmtKind::Test { .. } | StmtKind::TestBlock { .. } | StmtKind::Import { .. } => Ok(Flow::Normal(None)),
            StmtKind::Assert(e) => {
                if self.truthy(e)? {
                    Ok(Flow::Normal(None))
                } else {
                    Err(Error::runtime("assertion failed", e.span))
                }
            }
        }
    }

    /// Does `v` match `pat`? On a match, returns the fields a variant pattern binds.
    fn match_pat(&mut self, pat: &MatchPat, v: &Value) -> Result<Option<Vec<(Pattern, Value)>>> {
        match pat {
            MatchPat::Wild => Ok(Some(Vec::new())),
            MatchPat::Value(e) => match self.eval_raw(e)? {
                Value::Ctor(def) => Err(Error::runtime(
                    format!("`{}` has fields; write `{}({})` to match it", def.name(), def.name(), vec!["_"; def.variant().fields.len()].join(", ")),
                    e.span,
                )),
                p => Ok((&p == v).then(Vec::new)),
            },
            MatchPat::Variant { name, binds, span } => {
                let def = match self.lookup(name) {
                    Some(Value::Ctor(def)) => def,
                    Some(Value::Record(..)) => return Err(Error::runtime(format!("`{name}` has no fields; match it as `{name}:`"), *span)),
                    _ => return Err(Error::runtime(format!("unknown variant `{name}`"), *span)),
                };
                let Value::Record(vdef, fields) = v else {
                    return Err(Error::runtime(format!("can't match {} against the variant `{name}`", describe(v)), *span));
                };
                if !Rc::ptr_eq(&def.decl, &vdef.decl) {
                    return Err(Error::runtime(format!("`{name}` isn't a variant of `{}`", vdef.decl.name), *span));
                }
                let names: Vec<&str> = def.variant().fields.iter().map(|p| &*p.name).collect();
                if binds.len() != names.len() {
                    return Err(Error::runtime(
                        format!("`{name}` has {} field(s) ({}), the pattern lists {}", names.len(), names.join(", "), binds.len()),
                        *span,
                    ));
                }
                if def.index != vdef.index {
                    return Ok(None);
                }
                Ok(Some(binds.iter().cloned().zip(fields.iter().cloned()).collect()))
            }
        }
    }

    /// Binds a variant pattern's fields in the arm's scope, read-only.
    fn bind_fields(&mut self, binds: Vec<(Pattern, Value)>) -> Result<()> {
        for (pat, v) in binds {
            self.bind_pattern(&pat, v, Span::default())?;
            let mut names = Vec::new();
            pattern_names(&pat, &mut names);
            self.env.borrow_mut().bound.extend(names);
        }
        Ok(())
    }

    /// A `match` on an enum value without `_` must name every variant.
    fn check_exhaustive<'p>(&mut self, pats: impl Iterator<Item = &'p MatchPat>, v: &Value, span: Span) -> Result<()> {
        let Value::Record(def, _) = v else { return Ok(()) };
        if def.decl.is_struct {
            return Ok(());
        }
        let mut covered: Vec<&Name> = Vec::new();
        for p in pats {
            match p {
                MatchPat::Wild => return Ok(()),
                MatchPat::Variant { name, .. } => covered.push(name),
                MatchPat::Value(Expr { kind: ExprKind::Var(name), .. }) => covered.push(name),
                MatchPat::Value(_) => {}
            }
        }
        let missing: Vec<String> =
            def.decl.variants.iter().filter(|v| !covered.contains(&&v.name)).map(|v| format!("`{}`", v.name)).collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(Error::runtime(format!("this match doesn't handle {} of `{}` (add the arms, or `_`)", missing.join(", "), def.decl.name), span))
    }

    fn truthy(&mut self, e: &Expr) -> Result<bool> {
        match self.eval(e)? {
            Value::Bool(b) => Ok(b),
            other => Err(Error::runtime(format!("a condition must be a bool, found {}", describe(&other)), e.span)),
        }
    }

    // ---- expressions -------------------------------------------------------

    /// Evaluates an expression; a missing value is an error (a trap).
    pub fn eval(&mut self, e: &Expr) -> Result<Value> {
        let v = self.eval_raw(e)?;
        if let Value::Missing = v {
            let what = match &e.kind {
                ExprKind::Method { name, .. } => format!("`{name}()` found nothing"),
                _ => "no value here".to_string(),
            };
            return Err(Error::runtime(format!("{what}; to handle that case, use it in a condition: `if x = …:` / `while x = …:`"), e.span));
        }
        Ok(v)
    }

    /// Evaluates an expression that may produce a missing value (only for
    /// conditional bindings).
    pub fn eval_raw(&mut self, e: &Expr) -> Result<Value> {
        let span = e.span;
        Ok(match &e.kind {
            ExprKind::Int(n) => Value::Int(*n),
            ExprKind::Str(s) => Value::Str(s.clone()),
            ExprKind::Char(c) => Value::Char(*c),
            ExprKind::Bool(b) => Value::Bool(*b),
            ExprKind::Var(n) if &**n == "it" && self.lookup(n).is_none() => {
                return Err(Error::runtime("`it` only works inside an argument of a lambda-taking method (`.map(it * 2)`)", span));
            }
            ExprKind::Var(n) => self.lookup(n).ok_or_else(|| Error::runtime(format!("unknown name `{n}`"), span))?,
            ExprKind::Path(t, m) => match (&**t, &**m) {
                ("int", "Max") => Value::Int(i64::MAX),
                ("int", "Min") => Value::Int(i64::MIN),
                _ => return Err(Error::runtime(format!("unknown constant `{t}::{m}`"), span)),
            },
            ExprKind::Tuple(items) => Value::tuple(items.iter().map(|x| self.eval(x)).collect::<Result<_>>()?),
            ExprKind::List(items) => Value::List(Rc::new(items.iter().map(|x| self.eval(x)).collect::<Result<_>>()?)),
            ExprKind::Unary(op, x) => {
                let v = self.eval(x)?;
                builtins::unary(*op, v, span)?
            }
            ExprKind::Cast(x, ty) => {
                let v = self.eval(x)?;
                builtins::cast(v, ty, span)?
            }
            ExprKind::Binary(op, l, r) => {
                let a = self.eval(l)?;
                let b = self.eval(r)?;
                builtins::binary(*op, a, b, span)?
            }
            ExprKind::And(l, r) => Value::Bool(self.truthy(l)? && self.truthy(r)?),
            ExprKind::Or(l, r) => Value::Bool(self.truthy(l)? || self.truthy(r)?),
            ExprKind::In { item, coll, negated } => {
                let i = self.eval(item)?;
                let c = self.eval(coll)?;
                Value::Bool(builtins::contains(&c, &i, span)? != *negated)
            }
            ExprKind::Range { start, end, inclusive } => {
                let int = |s: &mut Self, x: &Expr| match s.eval(x)? {
                    Value::Int(n) => Ok(n),
                    other => Err(Error::runtime(format!("range bounds must be ints, found {}", describe(&other)), x.span)),
                };
                let start = match start {
                    Some(x) => Some(int(self, x)?),
                    None => None,
                };
                let end = match end {
                    Some(x) => {
                        let n = int(self, x)?;
                        Some(if *inclusive { n.checked_add(1).ok_or_else(|| Error::runtime("integer overflow", span))? } else { n })
                    }
                    None => None,
                };
                Value::Range { start, end }
            }
            ExprKind::Call { name, args } => self.call_named(name, args, span)?,
            ExprKind::Method { recv, name, args } => self.call_method(recv, name, args, span)?,
            ExprKind::Index { recv, index } => {
                let c = self.eval(recv)?;
                let k = self.eval(index)?;
                builtins::index(&c, &k, index.span)?
            }
            ExprKind::Field { recv, name } => {
                let r = self.eval(recv)?;
                field(&r, name, span)?
            }
            ExprKind::Lambda(l) => Value::Lambda(Rc::new(LambdaV { ast: l.clone(), env: self.env.clone() })),
            ExprKind::IfElse(c, a, b) => {
                if self.truthy(c)? {
                    self.eval(a)?
                } else {
                    self.eval(b)?
                }
            }
            ExprKind::Match(subject, arms) => {
                let v = self.eval(subject)?;
                self.check_exhaustive(arms.iter().map(|(p, _)| p), &v, subject.span)?;
                for (pat, value) in arms {
                    if let Some(binds) = self.match_pat(pat, &v)? {
                        let scope = new_scope(Some(self.env.clone()));
                        return self.in_scope(scope, |s| {
                            s.bind_fields(binds)?;
                            s.eval(value)
                        });
                    }
                }
                return Err(Error::runtime(format!("no match arm for {}", v.repr()), subject.span));
            }
            ExprKind::Ref(_) => return Err(Error::runtime("`&` is only allowed on an argument for an `inout` parameter", span)),
        })
    }

    fn eval_args(&mut self, args: &[Arg]) -> Result<Args> {
        let mut out = Args::default();
        for a in args {
            let v = self.eval(&a.value)?;
            match &a.name {
                Some(n) => out.named.push((n.clone(), v)),
                None => out.pos.push(v),
            }
            out.spans.push(a.value.span);
        }
        Ok(out)
    }

    /// Turns a chain into a list (needed before borrowing a place mutably).
    pub fn materialize(&mut self, v: Value) -> Result<Value> {
        match v {
            Value::Chain(c) => {
                let mut items = Vec::new();
                self.drive(&c, &mut |_, x| {
                    items.push(x);
                    Ok(true)
                })?;
                Ok(Value::List(Rc::new(items)))
            }
            other => Ok(other),
        }
    }

    // ---- calls -------------------------------------------------------------

    fn call_named(&mut self, name: &Name, args: &[Arg], span: Span) -> Result<Value> {
        if let Some(v) = self.lookup(name) {
            return match v {
                Value::Func(f) => self.call_func(&f, args, span),
                Value::Ctor(def) => self.construct(&def, args, span),
                Value::Record(..) => Err(Error::runtime(format!("`{name}` has no fields; write `{name}` without parentheses"), span)),
                Value::Lambda(_) => Err(Error::runtime(format!("`{name}` is a lambda; lambdas can't be stored or called by name"), span)),
                other => Err(Error::runtime(format!("`{name}` is {}, not a function", describe(&other)), span)),
            };
        }
        let a = self.eval_args(args)?;
        builtins::call_global(self, name, a, span)
    }

    fn call_method(&mut self, recv: &Expr, name: &Name, args: &[Arg], span: Span) -> Result<Value> {
        if &**name == "sort" {
            // In-place sort: the `by` lambda runs before the place is borrowed.
            let a = self.eval_args(args)?;
            let current = self.eval(recv)?;
            let Value::List(_) = current else {
                return Err(Error::runtime(format!("sort() sorts a list in place; for {} use .to_sorted()", describe(&current)), span));
            };
            let chain = builtins::to_chain(&current, span)?;
            let sorted = builtins::sorted(self, &chain, &a, "sort", span)?;
            let place = self.place(recv)?.ok_or_else(|| Error::runtime("sort() changes a list in place, so it needs a variable (or use .to_sorted())", span))?;
            self.with_place(&place, false, |slot| {
                *slot = Value::List(Rc::new(sorted));
                Ok(())
            })?;
            return Ok(Value::Unit);
        }
        if builtins::is_mutating(name) {
            let mut a = self.eval_args(args)?;
            for v in a.pos.iter_mut() {
                *v = self.materialize(std::mem::replace(v, Value::Unit))?;
            }
            if let Some(place) = self.place(recv)? {
                return self.with_place(&place, false, |slot| builtins::call_mut(slot, name, a, span));
            }
            let mut v = self.eval(recv)?;
            return builtins::call_mut(&mut v, name, a, span);
        }
        let r = self.eval(recv)?;
        let a = self.eval_args(args)?;
        builtins::call_method(self, r, name, a, span)
    }

    fn call_func(&mut self, f: &Rc<Func>, args: &[Arg], span: Span) -> Result<Value> {
        let decl = f.decl.clone();
        let mut values: Vec<Option<Value>> = vec![None; decl.params.len()];
        let mut refs: Vec<(usize, Place)> = Vec::new();
        for (k, arg) in args.iter().enumerate() {
            let idx = match &arg.name {
                Some(n) => decl
                    .params
                    .iter()
                    .position(|p| &p.name == n)
                    .ok_or_else(|| Error::runtime(format!("`{}` has no parameter named `{n}`", decl.name), arg.value.span))?,
                None if k < decl.params.len() => k,
                None => return Err(Error::runtime(format!("too many arguments for `{}`", decl.name), arg.value.span)),
            };
            if values[idx].is_some() {
                return Err(Error::runtime(format!("parameter `{}` given twice", decl.params[idx].name), arg.value.span));
            }
            let param = &decl.params[idx];
            let v = if let ExprKind::Ref(inner) = &arg.value.kind {
                if !param.inout {
                    return Err(Error::runtime(format!("`&` passed to `{}`, which isn't an inout parameter", param.name), arg.value.span));
                }
                let place = self
                    .place(inner)?
                    .ok_or_else(|| Error::runtime("`&` needs a variable or an element of one", inner.span))?;
                let v = self.eval(inner)?;
                refs.push((idx, place));
                v
            } else {
                if param.inout {
                    return Err(Error::runtime(format!("`{}` is inout; pass it as `&{}`", param.name, param.name), arg.value.span));
                }
                self.eval(&arg.value)?
            };
            if let Value::Chain(_) = v {
                return Err(Error::runtime("a chain can't be passed to a function; use .to_list() first", arg.value.span));
            }
            values[idx] = Some(v);
        }
        let mut full = Vec::with_capacity(values.len());
        for (p, v) in decl.params.iter().zip(values) {
            full.push(match (v, &p.default) {
                (Some(v), _) => v,
                (None, Some(d)) => self.in_env(f.env.clone(), |s| s.eval(d))?,
                (None, None) => return Err(Error::runtime(format!("missing argument `{}` for `{}`", p.name, decl.name), span)),
            });
        }
        for (p, v) in decl.params.iter().zip(full.iter()) {
            if !types::fits(v, &p.ty) {
                return Err(Error::runtime(
                    format!("`{}` expects `{}: {}`, got {}", decl.name, p.name, p.ty, types::describe_typed(v)),
                    span,
                ));
            }
        }
        if decl.memo {
            if !refs.is_empty() {
                return Err(Error::runtime("a memo fun can't take inout parameters", span));
            }
            if let Some(v) = f.cache.borrow().get(&full) {
                return Ok(v.clone());
            }
        }
        let call_env = new_boundary(Some(f.env.clone()));
        {
            let mut scope = call_env.borrow_mut();
            for (p, v) in decl.params.iter().zip(full.iter()) {
                scope.vars.insert(p.name.clone(), v.clone());
                if !p.inout {
                    scope.readonly.insert(p.name.clone());
                }
            }
        }
        let flow = self.in_scope(call_env.clone(), |s| s.exec_stmts(&decl.body))?;
        let explicit_return = matches!(flow, Flow::Return(_));
        let result = match flow {
            Flow::Return(v) | Flow::Normal(Some(v)) => v,
            Flow::Normal(None) => Value::Unit,
            Flow::Break | Flow::Continue => return Err(Error::runtime("`break`/`continue` outside a loop", decl.span)),
        };
        let result = match &decl.ret {
            Some(t) if !types::fits(&result, t) => {
                let got = if matches!(result, Value::Unit) { "nothing".to_string() } else { types::describe_typed(&result) };
                let hint = if matches!(result, Value::Unit) && ends_with_block_if(&decl.body) { BLOCK_IF_HINT } else { "" };
                return Err(Error::runtime(format!("`{}` should return {t}, but returned {got}{hint}", decl.name), span));
            }
            Some(_) => result,
            // No `-> T`: the function returns nothing, whatever its last line evaluates to.
            None if explicit_return && !matches!(result, Value::Unit) => {
                return Err(Error::runtime(
                    format!("`{}` returns a value but its signature has no `-> T`", decl.name),
                    span,
                ));
            }
            None => Value::Unit,
        };
        for (idx, place) in refs {
            let v = call_env.borrow().vars.get(&decl.params[idx].name).cloned().unwrap();
            self.with_place(&place, true, |slot| {
                *slot = v;
                Ok(())
            })?;
        }
        if decl.memo {
            f.cache.borrow_mut().insert(full, result.clone());
        }
        Ok(result)
    }

    pub fn call_value(&mut self, f: &Value, args: Vec<Value>, span: Span) -> Result<Value> {
        match f {
            Value::Lambda(l) => {
                let params = &l.ast.params;
                let args = if params.len() == args.len() {
                    args
                } else if let ([Value::Tuple(t)], true) = (args.as_slice(), params.len() > 1) {
                    if t.len() != params.len() {
                        return Err(Error::runtime(format!("lambda takes {} parameters, got a tuple of {}", params.len(), t.len()), span));
                    }
                    t.to_vec()
                } else {
                    return Err(Error::runtime(format!("lambda takes {} parameters, got {}", params.len(), args.len()), span));
                };
                let scope = new_boundary(Some(l.env.clone()));
                let l = l.clone();
                self.in_scope(scope, |s| {
                    for (p, v) in l.ast.params.iter().zip(args) {
                        s.bind_pattern(p, v, l.ast.body.span)?;
                    }
                    s.eval(&l.ast.body)
                })
            }
            Value::Func(func) => Err(Error::runtime(
                format!("functions aren't values; write `{}(it)`", func.decl.name),
                span,
            )),
            other => Err(Error::runtime(format!("expected a lambda, found {}", describe(other)), span)),
        }
    }

    // ---- chains ------------------------------------------------------------

    /// Pushes every element of `chain` through its stages into `sink`, one at
    /// a time. `sink` returns false to stop early. Returns false if stopped.
    pub fn drive(&mut self, chain: &ChainV, sink: &mut dyn FnMut(&mut Interp, Value) -> Result<bool>) -> Result<bool> {
        let mut states: Vec<StageState> = chain
            .stages
            .iter()
            .map(|s| match s {
                StageOp::Enumerate(start) => StageState::Count(*start),
                StageOp::Scan(init, _) => StageState::Acc(init.clone()),
                StageOp::Take(_) | StageOp::Skip(_) | StageOp::Zip(_) => StageState::Count(0),
                StageOp::Chunks(_) | StageOp::Windows(_) => StageState::Buf(VecDeque::new()),
                _ => StageState::None,
            })
            .collect();
        let stages = &chain.stages;
        let span = chain.span;
        if !self.pump(&chain.source, stages, &mut states, span, sink)? {
            return Ok(false);
        }
        // The source has run out: hand on each unfinished chunk, in stage order
        // (a flushed chunk may fill a later stage's chunk, which then flushes too).
        for i in 0..stages.len() {
            if let (StageOp::Chunks(_), StageState::Buf(buf)) = (&stages[i], &mut states[i]) {
                if !buf.is_empty() {
                    let chunk = Value::List(Rc::new(std::mem::take(buf).into()));
                    if !self.feed(stages, i + 1, chunk, &mut states, span, sink)? {
                        return Ok(false);
                    }
                }
            }
        }
        // Every element went through: a zip whose other side has some left over had unequal lengths.
        for (stage, state) in stages.iter().zip(&states) {
            if let (StageOp::Zip(other), StageState::Count(n)) = (stage, state) {
                if (*n as usize) < other.len() {
                    return Err(zip_mismatch(*n as usize, other.len(), span));
                }
            }
        }
        Ok(true)
    }

    /// Feeds every element of a chain's source into its first stage.
    fn pump(
        &mut self,
        source: &Source,
        stages: &[StageOp],
        states: &mut [StageState],
        span: Span,
        sink: &mut dyn FnMut(&mut Interp, Value) -> Result<bool>,
    ) -> Result<bool> {
        match source {
            Source::Values { items, rev } => {
                let items = items.clone();
                let n = items.len();
                for k in 0..n {
                    let v = items[if *rev { n - 1 - k } else { k }].clone();
                    if !self.feed(stages, 0, v, states, span, sink)? {
                        return Ok(false);
                    }
                }
            }
            Source::Chars { s, rev } => {
                let s = s.clone();
                let bytes = s.as_bytes();
                let n = bytes.len();
                for k in 0..n {
                    let b = bytes[if *rev { n - 1 - k } else { k }];
                    if !self.feed(stages, 0, Value::Char(b), states, span, sink)? {
                        return Ok(false);
                    }
                }
            }
            Source::Range { start, end, rev } => {
                if *rev {
                    let mut i = end.expect("reversed ranges have an end") - 1;
                    while i >= *start {
                        if !self.feed(stages, 0, Value::Int(i), states, span, sink)? {
                            return Ok(false);
                        }
                        i -= 1;
                    }
                } else {
                    let mut i = *start;
                    while end.is_none_or(|e| i < e) {
                        if !self.feed(stages, 0, Value::Int(i), states, span, sink)? {
                            return Ok(false);
                        }
                        i = i.checked_add(1).ok_or_else(|| Error::runtime("range overflowed int", span))?;
                    }
                }
            }
            Source::Arrangements { items, k } => {
                let items = items.clone();
                let n = items.len();
                let pick = |idx: &[usize]| Value::List(Rc::new(idx.iter().map(|&i| items[i].clone()).collect()));
                match k {
                    None => {
                        let mut idx: Vec<usize> = (0..n).collect();
                        loop {
                            if !self.feed(stages, 0, pick(&idx), states, span, sink)? {
                                return Ok(false);
                            }
                            // Next ordering of the positions: the rightmost rise, its next
                            // larger value to the right swapped in, and the tail sorted.
                            let Some(i) = (1..n).rev().find(|&i| idx[i - 1] < idx[i]) else { break };
                            let j = (i..n).rev().find(|&j| idx[j] > idx[i - 1]).unwrap();
                            idx.swap(i - 1, j);
                            idx[i..].reverse();
                        }
                    }
                    Some(k) if *k <= n => {
                        let k = *k;
                        let mut idx: Vec<usize> = (0..k).collect();
                        loop {
                            if !self.feed(stages, 0, pick(&idx), states, span, sink)? {
                                return Ok(false);
                            }
                            // Next subset: the rightmost position that can still move right.
                            let Some(i) = (0..k).rev().find(|&i| idx[i] < n - k + i) else { break };
                            idx[i] += 1;
                            for j in i + 1..k {
                                idx[j] = idx[j - 1] + 1;
                            }
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        Ok(true)
    }

    fn feed(
        &mut self,
        stages: &[StageOp],
        i: usize,
        item: Value,
        states: &mut [StageState],
        span: Span,
        sink: &mut dyn FnMut(&mut Interp, Value) -> Result<bool>,
    ) -> Result<bool> {
        if i == stages.len() {
            return sink(self, item);
        }
        match &stages[i] {
            StageOp::Map(f) => {
                let v = self.call_value(f, vec![item], span)?;
                self.feed(stages, i + 1, v, states, span, sink)
            }
            StageOp::Filter(f) => match self.call_value(f, vec![item.clone()], span)? {
                Value::Bool(true) => self.feed(stages, i + 1, item, states, span, sink),
                Value::Bool(false) => Ok(true),
                other => Err(Error::runtime(format!("filter's lambda must return a bool, got {}", describe(&other)), span)),
            },
            StageOp::FlatMap(f) => {
                let inner = self.call_value(f, vec![item], span)?;
                let inner = builtins::to_chain(&inner, span)?;
                self.drive(&inner, &mut |s, v| s.feed(stages, i + 1, v, &mut *states, span, &mut *sink))
            }
            StageOp::Enumerate(_) => {
                let StageState::Count(n) = &mut states[i] else { unreachable!() };
                let idx = *n;
                *n += 1;
                self.feed(stages, i + 1, Value::tuple(vec![Value::Int(idx), item]), states, span, sink)
            }
            StageOp::Zip(other) => {
                let StageState::Count(n) = &mut states[i] else { unreachable!() };
                let idx = *n as usize;
                let Some(y) = other.get(idx) else {
                    return Err(zip_mismatch(idx + 1, other.len(), span));
                };
                *n += 1;
                self.feed(stages, i + 1, Value::tuple(vec![item, y.clone()]), states, span, sink)
            }
            StageOp::Scan(_, f) => {
                let StageState::Acc(acc) = &states[i] else { unreachable!() };
                let acc = acc.clone();
                let next = self.call_value(f, vec![acc, item], span)?;
                states[i] = StageState::Acc(next.clone());
                self.feed(stages, i + 1, next, states, span, sink)
            }
            StageOp::Take(n) => {
                let StageState::Count(taken) = &mut states[i] else { unreachable!() };
                if *taken >= *n {
                    return Ok(false);
                }
                *taken += 1;
                let more = *taken < *n;
                // Stop right after the n-th element, so nothing extra is pulled.
                Ok(self.feed(stages, i + 1, item, states, span, sink)? && more)
            }
            StageOp::Skip(n) => {
                let StageState::Count(skipped) = &mut states[i] else { unreachable!() };
                if *skipped < *n {
                    *skipped += 1;
                    return Ok(true);
                }
                self.feed(stages, i + 1, item, states, span, sink)
            }
            StageOp::Intersect(other) => {
                if builtins::contains(other, &item, span)? {
                    self.feed(stages, i + 1, item, states, span, sink)
                } else {
                    Ok(true)
                }
            }
            StageOp::Chunks(n) => {
                let StageState::Buf(buf) = &mut states[i] else { unreachable!() };
                buf.push_back(item);
                if buf.len() < *n {
                    return Ok(true);
                }
                let chunk = Value::List(Rc::new(std::mem::take(buf).into()));
                self.feed(stages, i + 1, chunk, states, span, sink)
            }
            StageOp::Windows(n) => {
                let StageState::Buf(buf) = &mut states[i] else { unreachable!() };
                buf.push_back(item);
                if buf.len() > *n {
                    buf.pop_front();
                }
                if buf.len() < *n {
                    return Ok(true);
                }
                let window = Value::List(Rc::new(buf.iter().cloned().collect()));
                self.feed(stages, i + 1, window, states, span, sink)
            }
        }
    }
}

pub enum StageState {
    None,
    Count(i64),
    Acc(Value),
    Buf(VecDeque<Value>),
}

fn zip_mismatch(left: usize, right: usize, span: Span) -> Error {
    let left = if left > right { format!("at least {left}") } else { left.to_string() };
    Error::runtime(format!("zip needs sequences of equal length, got {left} and {right} elements"), span)
}

const BLOCK_IF_HINT: &str = "; a block `if` is a statement, so write `if c then a else b` to get a value";

fn ends_with_block_if(body: &[Stmt]) -> bool {
    body.iter().rev().find(|s| !is_test(s)).is_some_and(|s| matches!(s.kind, StmtKind::If { .. }))
}

pub fn is_test(s: &Stmt) -> bool {
    matches!(s.kind, StmtKind::Test { .. } | StmtKind::TestBlock { .. })
}

fn pattern_names(p: &Pattern, out: &mut Vec<Name>) {
    match p {
        Pattern::Name(n) => out.push(n.clone()),
        Pattern::Wild => {}
        Pattern::Tuple(ps) | Pattern::List(ps) => ps.iter().for_each(|p| pattern_names(p, out)),
    }
}

fn field_index(v: &Value, name: &str, span: Span) -> Result<usize> {
    match v {
        Value::Record(def, _) if def.decl.is_struct => def
            .variant()
            .fields
            .iter()
            .position(|p| &*p.name == name)
            .ok_or_else(|| Error::runtime(format!("`{}` has no field `{name}`", def.decl.name), span)),
        Value::Record(def, _) => Err(Error::runtime(
            format!("`.{name}` on the enum `{}`: a variant's fields are read with `match`", def.decl.name),
            span,
        )),
        other => Err(Error::runtime(
            format!("`.{name}` reads a struct field, but this is {} (methods need parentheses: `.{name}()`)", describe(other)),
            span,
        )),
    }
}

fn field(v: &Value, name: &str, span: Span) -> Result<Value> {
    let i = field_index(v, name, span)?;
    let Value::Record(_, fields) = v else { unreachable!() };
    Ok(fields[i].clone())
}

fn field_mut<'a>(v: &'a mut Value, name: &str, span: Span) -> Result<&'a mut Value> {
    let i = field_index(v, name, span)?;
    let Value::Record(_, fields) = v else { unreachable!() };
    Ok(&mut Rc::make_mut(fields)[i])
}

pub fn describe(v: &Value) -> String {
    match v {
        Value::Record(def, _) => format!("{} {}", if def.decl.is_struct { "a struct" } else { "the variant" }, def.name()),
        Value::Int(_) | Value::Char(_) | Value::Bool(_) => format!("{} {}", v.type_name(), v.repr()),
        Value::Str(s) if s.len() <= 20 => format!("string {}", v.repr()),
        _ => format!("a {}", v.type_name()),
    }
}
