//! The standard library: operators, indexing, global functions and methods.

use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use crate::ast::{BinOp, Name, UnOp};
use crate::error::{Error, Result, Span};
use crate::interp::{Interp, describe};
use crate::value::*;

const UP: (i64, i64) = (-1, 0);
const RIGHT: (i64, i64) = (0, 1);
const DOWN: (i64, i64) = (1, 0);
const LEFT: (i64, i64) = (0, -1);
const DIRS4: [(i64, i64); 4] = [UP, RIGHT, DOWN, LEFT];
const DIRS8: [(i64, i64); 8] = [(-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1)];

pub fn globals() -> Env {
    let env = new_scope(None);
    {
        let mut s = env.borrow_mut();
        for (name, (r, c)) in [("Up", UP), ("Right", RIGHT), ("Down", DOWN), ("Left", LEFT)] {
            s.vars.insert(name.into(), Value::point(r, c));
        }
        s.vars.insert("Dirs4".into(), Value::List(Rc::new(DIRS4.iter().map(|&(r, c)| Value::point(r, c)).collect())));
        s.vars.insert("Dirs8".into(), Value::List(Rc::new(DIRS8.iter().map(|&(r, c)| Value::point(r, c)).collect())));
    }
    env
}

// ---- arguments --------------------------------------------------------------

#[derive(Default)]
pub struct Args {
    pub pos: Vec<Value>,
    pub named: Vec<(Name, Value)>,
    pub spans: Vec<Span>,
}

impl Args {
    /// Positional argument `i`, or the named argument `name`.
    fn get(&self, i: usize, name: &str) -> Option<&Value> {
        self.named.iter().find(|(n, _)| &**n == name).map(|(_, v)| v).or_else(|| self.pos.get(i))
    }

    fn req(&self, i: usize, name: &str, method: &str, span: Span) -> Result<Value> {
        self.get(i, name).cloned().ok_or_else(|| Error::runtime(format!("`{method}` needs the argument `{name}`"), span))
    }

    /// Checks there are at most `max` positional args and only `allowed` named ones.
    fn check(&self, max: usize, allowed: &[&str], method: &str, span: Span) -> Result<()> {
        if self.pos.len() > max {
            return Err(Error::runtime(format!("`{method}` takes at most {max} argument(s), got {}", self.pos.len()), span));
        }
        for (n, _) in &self.named {
            if !allowed.contains(&&**n) {
                return Err(Error::runtime(format!("`{method}` has no parameter named `{n}`"), span));
            }
        }
        Ok(())
    }
}

fn int_arg(v: &Value, what: &str, span: Span) -> Result<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(Error::runtime(format!("{what} must be an int, found {}", describe(other)), span)),
    }
}

fn overflow(span: Span) -> Error {
    Error::runtime("integer overflow", span)
}

// ---- operators --------------------------------------------------------------

pub fn unary(op: UnOp, v: Value, span: Span) -> Result<Value> {
    match (op, v) {
        (UnOp::Neg, Value::Int(n)) => n.checked_neg().map(Value::Int).ok_or_else(|| overflow(span)),
        (UnOp::Neg, Value::Tuple(t)) => {
            Ok(Value::tuple(t.iter().map(|x| unary(UnOp::Neg, x.clone(), span)).collect::<Result<_>>()?))
        }
        (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
        (UnOp::Neg, v) => Err(Error::runtime(format!("can't negate {}", describe(&v)), span)),
        (UnOp::Not, v) => Err(Error::runtime(format!("`!` needs a bool, found {}", describe(&v)), span)),
    }
}

fn op_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
    }
}

pub fn binary(op: BinOp, a: Value, b: Value, span: Span) -> Result<Value> {
    use BinOp::*;
    match op {
        Eq => Ok(Value::Bool(a == b)),
        Ne => Ok(Value::Bool(a != b)),
        Lt | Le | Gt | Ge => {
            let o = a
                .compare(&b)
                .ok_or_else(|| Error::runtime(format!("can't compare {} with {}", describe(&a), describe(&b)), span))?;
            Ok(Value::Bool(match op {
                Lt => o.is_lt(),
                Le => o.is_le(),
                Gt => o.is_gt(),
                _ => o.is_ge(),
            }))
        }
        Add | Sub | Mul | Div | Rem => arith(op, a, b, span),
    }
}

fn arith(op: BinOp, a: Value, b: Value, span: Span) -> Result<Value> {
    use BinOp::*;
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => {
            let (x, y) = (*x, *y);
            if matches!(op, Div | Rem) && y == 0 {
                return Err(Error::runtime("division by zero", span));
            }
            let r = match op {
                Add => x.checked_add(y),
                Sub => x.checked_sub(y),
                Mul => x.checked_mul(y),
                Div => x.checked_div(y),
                _ => x.checked_rem(y),
            };
            r.map(Value::Int).ok_or_else(|| overflow(span))
        }
        (Value::Tuple(x), Value::Tuple(y)) if matches!(op, Add | Sub) => {
            if x.len() != y.len() {
                return Err(Error::runtime(format!("can't {} tuples of different lengths", op_symbol(op)), span));
            }
            Ok(Value::tuple(x.iter().zip(y.iter()).map(|(p, q)| arith(op, p.clone(), q.clone(), span)).collect::<Result<_>>()?))
        }
        (Value::Tuple(x), Value::Int(_)) if op == Mul => {
            Ok(Value::tuple(x.iter().map(|p| arith(Mul, p.clone(), b.clone(), span)).collect::<Result<_>>()?))
        }
        (Value::Int(_), Value::Tuple(_)) if op == Mul => arith(Mul, b, a, span),
        (Value::Char(x), Value::Char(y)) if op == Sub => Ok(Value::Int(*x as i64 - *y as i64)),
        (Value::Str(x), Value::Str(y)) if op == Add => Ok(Value::Str(format!("{x}{y}").into())),
        (Value::Str(x), Value::Char(y)) if op == Add => Ok(Value::Str(format!("{x}{}", *y as char).into())),
        (Value::List(_), Value::List(_)) if op == Add => {
            Err(Error::runtime("lists can't be joined with `+`; append with `xs.push(x)`, or join with `[xs, ys].flat_map(it)`", span))
        }
        (Value::List(x), Value::Int(n)) if op == Mul => {
            if *n < 0 {
                return Err(Error::runtime("can't repeat a list a negative number of times", span));
            }
            let mut out = Vec::with_capacity(x.len() * *n as usize);
            for _ in 0..*n {
                out.extend(x.iter().cloned());
            }
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err(Error::runtime(format!("can't apply `{}` to {} and {}", op_symbol(op), describe(&a), describe(&b)), span)),
    }
}

// ---- membership and indexing --------------------------------------------------

pub fn contains(coll: &Value, item: &Value, span: Span) -> Result<bool> {
    Ok(match coll {
        Value::List(xs) | Value::Tuple(xs) | Value::Stack(xs) => xs.contains(item),
        Value::Queue(q) => q.contains(item),
        Value::Set(s) => s.contains(item),
        Value::Map(m) => m.map.contains_key(item),
        Value::Str(s) => match item {
            Value::Char(c) => s.as_bytes().contains(c),
            Value::Str(sub) => s.contains(&**sub),
            _ => return Err(Error::runtime(format!("can't look for {} in a string", describe(item)), span)),
        },
        Value::Grid(g) => match item.as_point() {
            Some((r, c)) => g.contains(r, c),
            None => return Err(Error::runtime(format!("`in grid` needs a (row, col) point, found {}", describe(item)), span)),
        },
        Value::Range { start, end } => match item {
            Value::Int(n) => start.is_none_or(|s| *n >= s) && end.is_none_or(|e| *n < e),
            _ => false,
        },
        Value::Chain(_) => return Err(Error::runtime("`in` doesn't work on a chain; use .any(x => x == …)", span)),
        other => return Err(Error::runtime(format!("`in` doesn't work on {}", describe(other)), span)),
    })
}

fn slice_bounds(start: Option<i64>, end: Option<i64>, len: usize, span: Span) -> Result<(usize, usize)> {
    let s = start.unwrap_or(0);
    let e = end.unwrap_or(len as i64);
    if s < 0 || e < s || e > len as i64 {
        return Err(Error::runtime(format!("slice {s}..{e} is out of bounds for length {len}"), span));
    }
    Ok((s as usize, e as usize))
}

fn list_index(i: i64, len: usize, span: Span) -> Result<usize> {
    if i < 0 || i as usize >= len {
        Err(Error::runtime(format!("index {i} is out of bounds for length {len}"), span))
    } else {
        Ok(i as usize)
    }
}

fn grid_pos(g: &GridV, k: &Value, span: Span) -> Result<usize> {
    let (r, c) = k
        .as_point()
        .ok_or_else(|| Error::runtime(format!("a grid is indexed by a (row, col) point, found {}", describe(k)), span))?;
    if !g.contains(r, c) {
        return Err(Error::runtime(format!("position ({r}, {c}) is outside the grid ({} rows × {} cols)", g.rows, g.cols), span));
    }
    Ok(g.idx(r, c))
}

pub fn index(c: &Value, k: &Value, span: Span) -> Result<Value> {
    match (c, k) {
        (Value::List(xs) | Value::Tuple(xs), Value::Int(i)) => Ok(xs[list_index(*i, xs.len(), span)?].clone()),
        (Value::List(xs), Value::Range { start, end }) => {
            let (s, e) = slice_bounds(*start, *end, xs.len(), span)?;
            Ok(Value::List(Rc::new(xs[s..e].to_vec())))
        }
        (Value::Str(s), Value::Int(i)) => Ok(Value::Char(s.as_bytes()[list_index(*i, s.len(), span)?])),
        (Value::Str(s), Value::Range { start, end }) => {
            let (a, b) = slice_bounds(*start, *end, s.len(), span)?;
            Ok(Value::Str(s[a..b].into()))
        }
        (Value::Grid(g), _) => Ok(g.cells[grid_pos(g, k, span)?].clone()),
        (Value::Map(m), _) => match m.map.get(k) {
            Some(v) => Ok(v.clone()),
            None => match &m.default {
                Some(d) => Ok(d.clone()),
                None => Err(Error::runtime(
                    format!("key {} is not in the map (check with `k in m`, or give the map a default: Map(default = …))", k.repr()),
                    span,
                )),
            },
        },
        _ => Err(Error::runtime(format!("can't index {} with {}", describe(c), describe(k)), span)),
    }
}

/// A mutable reference to `slot[k]`. With `create`, a missing map key is inserted.
pub fn index_mut<'a>(slot: &'a mut Value, k: &Value, span: Span, create: bool) -> Result<&'a mut Value> {
    match slot {
        Value::List(xs) => {
            let i = list_index(int_arg(k, "a list index", span)?, xs.len(), span)?;
            Ok(&mut Rc::make_mut(xs)[i])
        }
        Value::Grid(g) => {
            let i = grid_pos(g, k, span)?;
            Ok(&mut Rc::make_mut(g).cells[i])
        }
        Value::Map(m) => {
            let m = Rc::make_mut(m);
            if !m.map.contains_key(k) {
                let init = match (&m.default, create) {
                    (_, true) => Value::Unit,
                    (Some(d), false) => d.clone(),
                    (None, false) => {
                        return Err(Error::runtime(
                            format!("key {} is not in the map (give the map a default: Map(default = …))", k.repr()),
                            span,
                        ));
                    }
                };
                m.map.insert(k.clone(), init);
            }
            Ok(m.map.get_mut(k).unwrap())
        }
        Value::Tuple(_) => Err(Error::runtime("tuples can't be changed in place; build a new one", span)),
        Value::Str(_) => Err(Error::runtime("strings can't be changed in place; build a new one", span)),
        other => Err(Error::runtime(format!("can't index {} for assignment", describe(other)), span)),
    }
}

// ---- iteration ----------------------------------------------------------------

pub fn to_chain(v: &Value, span: Span) -> Result<Rc<ChainV>> {
    let values = |items: Vec<Value>| Source::Values { items: Rc::new(items), rev: false };
    let source = match v {
        Value::Chain(c) => return Ok(c.clone()),
        Value::List(xs) | Value::Tuple(xs) | Value::Stack(xs) => Source::Values { items: xs.clone(), rev: false },
        Value::Queue(q) => values(q.iter().cloned().collect()),
        Value::Set(s) => values(s.iter().cloned().collect()),
        Value::Map(m) => values(m.map.iter().map(|(k, v)| Value::tuple(vec![k.clone(), v.clone()])).collect()),
        Value::Str(s) => Source::Chars { s: s.clone(), rev: false },
        Value::Range { start: Some(s), end } => Source::Range { start: *s, end: *end, rev: false },
        Value::Range { start: None, .. } => return Err(Error::runtime("a range needs a start to be iterated", span)),
        Value::Grid(_) => return Err(Error::runtime("a grid isn't iterable; use grid.positions()", span)),
        other => return Err(Error::runtime(format!("{} is not iterable", describe(other)), span)),
    };
    Ok(Rc::new(ChainV { source, stages: Vec::new(), span }))
}

fn chain_of(items: Vec<Value>, span: Span) -> Value {
    Value::Chain(Rc::new(ChainV { source: Source::Values { items: Rc::new(items), rev: false }, stages: Vec::new(), span }))
}

/// Elements of a non-chain iterable (chains are materialized before mutation).
fn values_of(v: &Value, span: Span) -> Result<Vec<Value>> {
    match v {
        Value::List(xs) | Value::Tuple(xs) | Value::Stack(xs) => Ok(xs.to_vec()),
        Value::Queue(q) => Ok(q.iter().cloned().collect()),
        Value::Set(s) => Ok(s.iter().cloned().collect()),
        Value::Str(s) => Ok(s.bytes().map(Value::Char).collect()),
        other => Err(Error::runtime(format!("expected a list or chain, found {}", describe(other)), span)),
    }
}

// ---- global functions ------------------------------------------------------------

pub fn call_global(it: &mut Interp, name: &str, a: Args, span: Span) -> Result<Value> {
    match name {
        "panic" => {
            let msg = a.pos.first().map(|v| v.to_string()).unwrap_or_else(|| "panic".into());
            Err(Error::runtime(format!("panic: {msg}"), span))
        }
        "print" => {
            let text: Vec<String> = a.pos.iter().map(|v| v.to_string()).collect();
            eprintln!("{}", text.join(" "));
            Ok(Value::Unit)
        }
        "min" | "max" => {
            a.check(2, &[], name, span)?;
            let x = a.req(0, "a", name, span)?;
            let y = a.req(1, "b", name, span)?;
            let o = x.compare(&y).ok_or_else(|| Error::runtime(format!("can't compare {} with {}", describe(&x), describe(&y)), span))?;
            Ok(if (name == "min") == o.is_le() { x } else { y })
        }
        "abs" => {
            let n = int_arg(&a.req(0, "n", name, span)?, "abs's argument", span)?;
            n.checked_abs().map(Value::Int).ok_or_else(|| overflow(span))
        }
        "Set" => {
            a.check(1, &[], name, span)?;
            let mut s = HashSet::new();
            if let Some(x) = a.pos.first() {
                s.insert(x.clone());
            }
            Ok(Value::Set(Rc::new(s)))
        }
        "Map" => {
            a.check(0, &["default"], name, span)?;
            Ok(Value::Map(Rc::new(MapV { map: Default::default(), default: a.get(usize::MAX, "default").cloned() })))
        }
        "Queue" => {
            a.check(1, &[], name, span)?;
            Ok(Value::Queue(Rc::new(a.pos.first().cloned().into_iter().collect::<VecDeque<_>>())))
        }
        "Stack" => {
            a.check(1, &[], name, span)?;
            Ok(Value::Stack(Rc::new(a.pos.first().cloned().into_iter().collect())))
        }
        "MinQueue" => {
            a.check(2, &[], name, span)?;
            let mut q = MinQ::default();
            if let Some(item) = a.pos.first() {
                let prio = int_arg(&a.req(1, "priority", name, span)?, "the priority", span)?;
                q.heap.push(HeapEntry { prio, seq: 0, item: item.clone() });
                q.seq = 1;
            }
            Ok(Value::MinQueue(Rc::new(q)))
        }
        "Grid" => {
            a.check(3, &["fill"], name, span)?;
            let rows = int_arg(&a.req(0, "rows", name, span)?, "rows", span)?;
            let cols = int_arg(&a.req(1, "cols", name, span)?, "cols", span)?;
            let fill = a.req(2, "fill", name, span)?;
            if rows < 0 || cols < 0 {
                return Err(Error::runtime("a grid can't have a negative size", span));
            }
            Ok(Value::Grid(Rc::new(GridV { rows, cols, cells: vec![fill; (rows * cols) as usize] })))
        }
        "file" => {
            let p = match a.pos.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => return Err(Error::runtime("file(…) needs a path string", span)),
            };
            let path = it.base_dir.join(&*p);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| Error::runtime(format!("can't read {}: {e}", path.display()), span))?;
            Ok(Value::Str(text.into()))
        }
        _ => Err(Error::runtime(format!("unknown function `{name}`"), span)),
    }
}

// ---- mutating methods ---------------------------------------------------------------

pub fn is_mutating(name: &str) -> bool {
    matches!(name, "push" | "pop" | "add" | "remove" | "push_all" | "pop_many")
}

pub fn call_mut(slot: &mut Value, name: &str, a: Args, span: Span) -> Result<Value> {
    let arg0 = |what: &str| a.pos.first().cloned().ok_or_else(|| Error::runtime(format!("`{name}` needs {what}"), span));
    let max_args = match (&*slot, name) {
        (Value::MinQueue(_), "push") => 2,
        (_, "pop") => 0,
        _ => 1,
    };
    if a.pos.len() > max_args {
        let hint = if name == "push" && max_args == 1 { "; to push a pair, wrap it in a tuple: `push((a, b))`" } else { "" };
        return Err(Error::runtime(format!("`{name}` on {} takes {max_args} argument{}, got {}{hint}", describe(slot), if max_args == 1 { "" } else { "s" }, a.pos.len()), span));
    }
    match (slot, name) {
        (Value::List(xs), "push") => {
            Rc::make_mut(xs).push(arg0("a value")?);
            Ok(Value::Unit)
        }
        (Value::List(xs), "pop") => Ok(Rc::make_mut(xs).pop().unwrap_or(Value::Missing)),
        (Value::List(xs), "push_all") => {
            let more = values_of(&arg0("a list")?, span)?;
            Rc::make_mut(xs).extend(more);
            Ok(Value::Unit)
        }
        (Value::Set(s), "add") => Ok(Value::Bool(Rc::make_mut(s).insert(arg0("a value")?))),
        (Value::Set(s), "remove") => Ok(Value::Bool(Rc::make_mut(s).remove(&arg0("a value")?))),
        (Value::Map(m), "remove") => Ok(Value::Bool(Rc::make_mut(m).map.remove(&arg0("a key")?).is_some())),
        (Value::Queue(q), "push") => {
            Rc::make_mut(q).push_back(arg0("a value")?);
            Ok(Value::Unit)
        }
        (Value::Queue(q), "pop") => Ok(Rc::make_mut(q).pop_front().unwrap_or(Value::Missing)),
        (Value::Queue(q), "push_all") => {
            let more = values_of(&arg0("a list")?, span)?;
            Rc::make_mut(q).extend(more);
            Ok(Value::Unit)
        }
        (Value::Stack(s), "push") => {
            Rc::make_mut(s).push(arg0("a value")?);
            Ok(Value::Unit)
        }
        (Value::Stack(s), "pop") => Ok(Rc::make_mut(s).pop().unwrap_or(Value::Missing)),
        (Value::Stack(s), "push_all") => {
            let more = values_of(&arg0("a list")?, span)?;
            Rc::make_mut(s).extend(more);
            Ok(Value::Unit)
        }
        (Value::Stack(s), "pop_many") => {
            let n = int_arg(&arg0("a count")?, "pop_many's count", span)?;
            if n < 0 || n as usize > s.len() {
                return Err(Error::runtime(format!("pop_many({n}) on a stack with {} elements", s.len()), span));
            }
            let s = Rc::make_mut(s);
            let rest = s.split_off(s.len() - n as usize);
            Ok(Value::List(Rc::new(rest)))
        }
        (Value::MinQueue(q), "push") => {
            let item = arg0("an item and a priority")?;
            let prio = int_arg(a.get(1, "priority").ok_or_else(|| Error::runtime("`push` on a MinQueue needs a priority", span))?, "the priority", span)?;
            let q = Rc::make_mut(q);
            q.heap.push(HeapEntry { prio, seq: q.seq, item });
            q.seq += 1;
            Ok(Value::Unit)
        }
        (Value::MinQueue(q), "pop") => Ok(match Rc::make_mut(q).heap.pop() {
            Some(e) => Value::tuple(vec![e.item, Value::Int(e.prio)]),
            None => Value::Missing,
        }),
        (v, _) => Err(Error::runtime(format!("{} has no method `{name}`", describe(v)), span)),
    }
}

// ---- other methods ----------------------------------------------------------------------

fn str_lines(s: &str) -> Vec<Value> {
    s.lines().map(|l| Value::Str(l.into())).collect()
}

/// All integers in `s`. A `-` is a sign only right before a digit and not right after one.
fn str_ints(s: &str, span: Span) -> Result<Vec<Value>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let neg = b[i] == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit) && (i == 0 || !b[i - 1].is_ascii_digit());
        if b[i].is_ascii_digit() || neg {
            let start = i;
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n = s[start..i].parse::<i64>().map_err(|_| Error::runtime(format!("number `{}` doesn't fit in an int", &s[start..i]), span))?;
            out.push(Value::Int(n));
        } else {
            i += 1;
        }
    }
    Ok(out)
}

fn parse_grid(s: &str, digits: bool, span: Span) -> Result<Value> {
    let lines: Vec<&str> = s.lines().collect();
    let rows = lines.len();
    let cols = lines.first().map_or(0, |l| l.len());
    let mut cells = Vec::with_capacity(rows * cols);
    for (r, line) in lines.iter().enumerate() {
        if line.len() != cols {
            return Err(Error::runtime(format!("ragged grid: line {} has {} chars, line 1 has {cols}", r + 1, line.len()), span));
        }
        for b in line.bytes() {
            cells.push(if digits {
                if !b.is_ascii_digit() {
                    return Err(Error::runtime(format!("to_digit_grid: `{}` on line {} is not a digit", b as char, r + 1), span));
                }
                Value::Int((b - b'0') as i64)
            } else {
                Value::Char(b)
            });
        }
    }
    Ok(Value::Grid(Rc::new(GridV { rows: rows as i64, cols: cols as i64, cells })))
}

pub fn call_method(it: &mut Interp, recv: Value, name: &str, a: Args, span: Span) -> Result<Value> {
    // Type-specific methods first.
    match (&recv, name) {
        (Value::Str(s), "len") => return Ok(Value::Int(s.len() as i64)),
        (Value::Str(s), "lines") => return Ok(chain_of(str_lines(s), span)),
        (Value::Str(s), "ints") => return Ok(chain_of(str_ints(s, span)?, span)),
        (Value::Str(s), "digits") => {
            return Ok(chain_of(s.bytes().filter(u8::is_ascii_digit).map(|b| Value::Int((b - b'0') as i64)).collect(), span));
        }
        (Value::Str(_), "chars") => return Ok(Value::Chain(to_chain(&recv, span)?)),
        (Value::Str(s), "starts_with") => {
            a.check(1, &["at"], name, span)?;
            let prefix = match a.get(0, "prefix") {
                Some(Value::Str(p)) => p.to_string(),
                Some(Value::Char(c)) => (*c as char).to_string(),
                _ => return Err(Error::runtime("starts_with needs a string", span)),
            };
            let at = match a.get(usize::MAX, "at") {
                Some(v) => int_arg(v, "`at`", span)?,
                None => 0,
            };
            return Ok(Value::Bool(at >= 0 && (at as usize) <= s.len() && s[at as usize..].starts_with(&prefix)));
        }
        (Value::Str(s), "split_once") => {
            let sep = match a.pos.first() {
                Some(Value::Str(p)) => p.to_string(),
                Some(Value::Char(c)) => (*c as char).to_string(),
                _ => return Err(Error::runtime("split_once needs a separator string", span)),
            };
            return match s.split_once(&sep) {
                Some((x, y)) => Ok(Value::tuple(vec![Value::Str(x.into()), Value::Str(y.into())])),
                None => Err(Error::runtime(format!("split_once: separator {sep:?} not found"), span)),
            };
        }
        (Value::Str(s), "split") => {
            let sep = match a.pos.first() {
                Some(Value::Str(p)) if !p.is_empty() => p.to_string(),
                Some(Value::Char(c)) => (*c as char).to_string(),
                _ => return Err(Error::runtime("split needs a non-empty separator string", span)),
            };
            return Ok(chain_of(s.split(&*sep).map(|part| Value::Str(part.into())).collect(), span));
        }
        (Value::Str(s), "int") => {
            let ok = s.strip_prefix('-').unwrap_or(s);
            if ok.is_empty() || !ok.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::runtime(format!("{} is not an integer (to pull numbers out of text, use .ints())", recv.repr()), span));
            }
            return s.parse::<i64>().map(Value::Int).map_err(|_| Error::runtime(format!("{} doesn't fit in an int", recv.repr()), span));
        }
        (Value::Str(s), "to_char_grid") => return parse_grid(s, false, span),
        (Value::Str(s), "to_digit_grid") => return parse_grid(s, true, span),
        (Value::Str(_), "to_string") => return Ok(recv.clone()),
        (Value::Char(c), "is_digit") => return Ok(Value::Bool(c.is_ascii_digit())),
        (Value::Char(c), "is_letter") => return Ok(Value::Bool(c.is_ascii_alphabetic())),
        (Value::Char(c), "digit") => {
            return if c.is_ascii_digit() {
                Ok(Value::Int((c - b'0') as i64))
            } else {
                Err(Error::runtime(format!("{} is not a digit", recv.repr()), span))
            };
        }
        (Value::Char(c), "dir") => {
            let (r, col) = match c {
                b'^' | b'U' => UP,
                b'>' | b'R' => RIGHT,
                b'v' | b'D' => DOWN,
                b'<' | b'L' => LEFT,
                _ => return Err(Error::runtime(format!("{} is not a direction (^ > v < or U R D L)", recv.repr()), span)),
            };
            return Ok(Value::point(r, col));
        }
        (Value::Char(c), "to_string") => return Ok(Value::Str((*c as char).to_string().into())),
        (Value::Int(n), "to_string") => return Ok(Value::Str(n.to_string().into())),
        (Value::Int(n), "digits") => {
            let digits = n.unsigned_abs().to_string().bytes().map(|b| Value::Int((b - b'0') as i64)).collect();
            return Ok(chain_of(digits, span));
        }
        (Value::List(xs) | Value::Tuple(xs) | Value::Stack(xs), "len") => return Ok(Value::Int(xs.len() as i64)),
        (Value::List(xs) | Value::Stack(xs), "is_empty") => return Ok(Value::Bool(xs.is_empty())),
        (Value::Stack(xs), "top") => return Ok(xs.last().cloned().unwrap_or(Value::Missing)),
        (Value::Set(s), "len") => return Ok(Value::Int(s.len() as i64)),
        (Value::Set(s), "is_empty") => return Ok(Value::Bool(s.is_empty())),
        (Value::Map(m), "len") => return Ok(Value::Int(m.map.len() as i64)),
        (Value::Map(m), "is_empty") => return Ok(Value::Bool(m.map.is_empty())),
        (Value::Map(m), "keys") => return Ok(chain_of(m.map.keys().cloned().collect(), span)),
        (Value::Map(m), "values") => return Ok(chain_of(m.map.values().cloned().collect(), span)),
        (Value::Queue(q), "len") => return Ok(Value::Int(q.len() as i64)),
        (Value::Queue(q), "is_empty") => return Ok(Value::Bool(q.is_empty())),
        (Value::MinQueue(q), "len") => return Ok(Value::Int(q.heap.len() as i64)),
        (Value::MinQueue(q), "is_empty") => return Ok(Value::Bool(q.heap.is_empty())),
        (Value::Str(s), "is_empty") => return Ok(Value::Bool(s.is_empty())),
        (Value::Grid(g), _) => return grid_method(g, name, &a, span),
        _ => {}
    }
    // Everything iterable gets the chain methods.
    if matches!(
        name,
        "map" | "filter" | "flat_map" | "enumerate" | "scan" | "take" | "skip" | "rev" | "sum" | "product" | "count" | "any" | "all" | "min"
            | "max" | "first" | "last" | "find" | "find_last" | "position" | "to_list" | "to_set" | "to_map" | "to_string"
            | "to_sorted" | "is_empty" | "chunks" | "windows" | "intersection"
    ) {
        let chain = to_chain(&recv, span)?;
        return chain_method(it, &chain, name, a, span);
    }
    Err(Error::runtime(format!("{} has no method `{name}`", describe(&recv)), span))
}

fn grid_method(g: &Rc<GridV>, name: &str, a: &Args, span: Span) -> Result<Value> {
    let point = |a: &Args| -> Result<(i64, i64)> {
        a.pos
            .first()
            .and_then(Value::as_point)
            .ok_or_else(|| Error::runtime(format!("`{name}` needs a (row, col) point"), span))
    };
    match name {
        "rows" => Ok(Value::Int(g.rows)),
        "cols" => Ok(Value::Int(g.cols)),
        "neighbors4" | "neighbors8" => {
            let (r, c) = point(a)?;
            let dirs: &[(i64, i64)] = if name == "neighbors4" { &DIRS4 } else { &DIRS8 };
            let items = dirs
                .iter()
                .map(|(dr, dc)| (r + dr, c + dc))
                .filter(|&(r, c)| g.contains(r, c))
                .map(|(r, c)| Value::point(r, c))
                .collect();
            Ok(chain_of(items, span))
        }
        "positions" => {
            let mut items = Vec::with_capacity(g.cells.len());
            for r in 0..g.rows {
                for c in 0..g.cols {
                    items.push(Value::point(r, c));
                }
            }
            Ok(chain_of(items, span))
        }
        "find" | "find_last" => {
            let target = a.pos.first().ok_or_else(|| Error::runtime(format!("`{name}` needs a value to look for"), span))?;
            let found = if name == "find" {
                g.cells.iter().position(|c| c == target)
            } else {
                g.cells.iter().rposition(|c| c == target)
            };
            Ok(match found {
                Some(i) => Value::point(i as i64 / g.cols, i as i64 % g.cols),
                None => Value::Missing,
            })
        }
        "print" => {
            eprintln!("{}\n", Value::Grid(g.clone()));
            Ok(Value::Unit)
        }
        "to_string" => Ok(Value::Str(Value::Grid(g.clone()).to_string().into())),
        _ => Err(Error::runtime(format!("a Grid has no method `{name}`"), span)),
    }
}

fn lambda_arg(a: &Args, i: usize, name: &str, method: &str, span: Span) -> Result<Value> {
    match a.get(i, name) {
        Some(v @ Value::Lambda(_)) => Ok(v.clone()),
        Some(Value::Func(f)) => {
            Err(Error::runtime(format!("functions aren't values; write `{}(it)`", f.decl.name), span))
        }
        Some(other) => Err(Error::runtime(format!("`{method}` needs a lambda, found {}", describe(other)), span)),
        None => Err(Error::runtime(format!("`{method}` needs a lambda"), span)),
    }
}

fn with_stage(chain: &ChainV, stage: StageOp, span: Span) -> Value {
    let mut stages = chain.stages.clone();
    stages.push(stage);
    Value::Chain(Rc::new(ChainV { source: chain.source.clone(), stages, span }))
}

fn truth(v: Value, method: &str, span: Span) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(b),
        other => Err(Error::runtime(format!("`{method}`'s lambda must return a bool, got {}", describe(&other)), span)),
    }
}

pub fn chain_method(it: &mut Interp, chain: &ChainV, name: &str, a: Args, span: Span) -> Result<Value> {
    match name {
        "map" => Ok(with_stage(chain, StageOp::Map(lambda_arg(&a, 0, "f", name, span)?), span)),
        "filter" => Ok(with_stage(chain, StageOp::Filter(lambda_arg(&a, 0, "f", name, span)?), span)),
        "flat_map" => Ok(with_stage(chain, StageOp::FlatMap(lambda_arg(&a, 0, "f", name, span)?), span)),
        "enumerate" => {
            a.check(1, &["start"], name, span)?;
            let start = match a.get(0, "start") {
                Some(v) => int_arg(v, "enumerate's start", span)?,
                None => 0,
            };
            Ok(with_stage(chain, StageOp::Enumerate(start), span))
        }
        "scan" => {
            a.check(2, &["init", "f"], name, span)?;
            let init = a.req(0, "init", name, span)?;
            Ok(with_stage(chain, StageOp::Scan(init, lambda_arg(&a, 1, "f", name, span)?), span))
        }
        "rev" => {
            if !chain.stages.is_empty() {
                return Err(Error::runtime("rev() works only on lists, strings and ranges, not on a chain with map/filter/…", span));
            }
            let source = match &chain.source {
                Source::Values { items, rev } => Source::Values { items: items.clone(), rev: !rev },
                Source::Chars { s, rev } => Source::Chars { s: s.clone(), rev: !rev },
                Source::Range { start, end: Some(e), rev } => Source::Range { start: *start, end: Some(*e), rev: !rev },
                Source::Range { end: None, .. } => return Err(Error::runtime("can't reverse a range without an end", span)),
            };
            Ok(Value::Chain(Rc::new(ChainV { source, stages: Vec::new(), span })))
        }
        "sum" => {
            let mut acc: Option<Value> = None;
            it.drive(chain, &mut |_, v| {
                acc = Some(match acc.take() {
                    None => v,
                    Some(x) => binary(BinOp::Add, x, v, span)?,
                });
                Ok(true)
            })?;
            Ok(acc.unwrap_or(Value::Int(0)))
        }
        "product" => {
            let mut acc = Value::Int(1);
            it.drive(chain, &mut |_, v| {
                if !matches!(v, Value::Int(_)) {
                    return Err(Error::runtime(format!("product() multiplies ints, found {}", describe(&v)), span));
                }
                acc = binary(BinOp::Mul, acc.clone(), v, span)?;
                Ok(true)
            })?;
            Ok(acc)
        }
        "count" => {
            let pred = a.pos.first().cloned();
            let mut n = 0i64;
            it.drive(chain, &mut |s, v| {
                let hit = match &pred {
                    None => true,
                    Some(f @ Value::Lambda(_)) => truth(s.call_value(f, vec![v], span)?, "count", span)?,
                    Some(x) => &v == x,
                };
                if hit {
                    n += 1;
                }
                Ok(true)
            })?;
            Ok(Value::Int(n))
        }
        "any" | "all" => {
            let f = lambda_arg(&a, 0, "f", name, span)?;
            let want = name == "any";
            let mut result = !want;
            it.drive(chain, &mut |s, v| {
                if truth(s.call_value(&f, vec![v], span)?, name, span)? == want {
                    result = want;
                    return Ok(false);
                }
                Ok(true)
            })?;
            Ok(Value::Bool(result))
        }
        "is_empty" => {
            let mut empty = true;
            it.drive(chain, &mut |_, _| {
                empty = false;
                Ok(false)
            })?;
            Ok(Value::Bool(empty))
        }
        "take" | "skip" => {
            a.check(1, &["n"], name, span)?;
            let n = int_arg(&a.req(0, "n", name, span)?, &format!("{name}'s count"), span)?;
            if n < 0 {
                return Err(Error::runtime(format!("{name}({n}): the count can't be negative"), span));
            }
            Ok(with_stage(chain, if name == "take" { StageOp::Take(n) } else { StageOp::Skip(n) }, span))
        }
        "chunks" | "windows" => {
            a.check(1, &["n"], name, span)?;
            let n = int_arg(&a.req(0, "n", name, span)?, &format!("{name}'s size"), span)?;
            if n < 1 {
                return Err(Error::runtime(format!("{name}({n}): the size must be at least 1"), span));
            }
            let n = n as usize;
            Ok(with_stage(chain, if name == "chunks" { StageOp::Chunks(n) } else { StageOp::Windows(n) }, span))
        }
        "intersection" => {
            a.check(1, &["other"], name, span)?;
            let other = a.req(0, "other", name, span)?;
            if matches!(other, Value::Chain(_)) {
                return Err(Error::runtime("intersection needs a string, list or Set to look in, not a chain; collect it with to_set()", span));
            }
            Ok(with_stage(chain, StageOp::Intersect(other), span))
        }
        "to_sorted" => Ok(Value::List(Rc::new(sorted(it, chain, &a, name, span)?))),
        "min" | "max" if a.get(usize::MAX, "count").is_some() => {
            a.check(0, &["count"], name, span)?;
            let n = int_arg(a.get(usize::MAX, "count").unwrap(), "count", span)?;
            let mut items = Vec::new();
            it.drive(chain, &mut |_, v| {
                items.push(v);
                Ok(true)
            })?;
            if n < 0 || n as usize > items.len() {
                return Err(Error::runtime(format!("{name}(count = {n}) on {} elements", items.len()), span));
            }
            let mut err = None;
            items.sort_by(|x, y| {
                let o = x.compare(y).unwrap_or_else(|| {
                    err.get_or_insert_with(|| Error::runtime(format!("can't compare {} with {}", describe(x), describe(y)), span));
                    std::cmp::Ordering::Equal
                });
                if name == "max" { o.reverse() } else { o }
            });
            if let Some(e) = err {
                return Err(e);
            }
            items.truncate(n as usize);
            Ok(Value::List(Rc::new(items)))
        }
        "min" | "max" => {
            a.check(0, &[], name, span)?;
            let mut best: Option<Value> = None;
            it.drive(chain, &mut |_, v| {
                best = Some(match best.take() {
                    None => v,
                    Some(b) => {
                        let o = v
                            .compare(&b)
                            .ok_or_else(|| Error::runtime(format!("can't compare {} with {}", describe(&v), describe(&b)), span))?;
                        if (name == "min" && o.is_lt()) || (name == "max" && o.is_gt()) { v } else { b }
                    }
                });
                Ok(true)
            })?;
            Ok(best.unwrap_or(Value::Missing))
        }
        "first" | "find" => {
            let f = if name == "find" { Some(lambda_arg(&a, 0, "f", name, span)?) } else { None };
            let mut found = Value::Missing;
            it.drive(chain, &mut |s, v| {
                let hit = match &f {
                    None => true,
                    Some(f) => truth(s.call_value(f, vec![v.clone()], span)?, name, span)?,
                };
                if hit {
                    found = v;
                    return Ok(false);
                }
                Ok(true)
            })?;
            Ok(found)
        }
        "last" | "find_last" => {
            let f = if name == "find_last" { Some(lambda_arg(&a, 0, "f", name, span)?) } else { None };
            let mut found = Value::Missing;
            it.drive(chain, &mut |s, v| {
                let hit = match &f {
                    None => true,
                    Some(f) => truth(s.call_value(f, vec![v.clone()], span)?, name, span)?,
                };
                if hit {
                    found = v;
                }
                Ok(true)
            })?;
            Ok(found)
        }
        "position" => {
            let f = lambda_arg(&a, 0, "f", name, span)?;
            let mut idx = 0i64;
            let mut found = Value::Missing;
            it.drive(chain, &mut |s, v| {
                if truth(s.call_value(&f, vec![v], span)?, name, span)? {
                    found = Value::Int(idx);
                    return Ok(false);
                }
                idx += 1;
                Ok(true)
            })?;
            Ok(found)
        }
        "to_list" | "to_set" | "to_map" | "to_string" => {
            let mut items = Vec::new();
            it.drive(chain, &mut |_, v| {
                items.push(v);
                Ok(true)
            })?;
            match name {
                "to_list" => Ok(Value::List(Rc::new(items))),
                "to_set" => Ok(Value::Set(Rc::new(items.into_iter().collect()))),
                "to_map" => {
                    let mut m = MapV::default();
                    for v in items {
                        match &v {
                            Value::Tuple(t) if t.len() == 2 => {
                                m.map.insert(t[0].clone(), t[1].clone());
                            }
                            _ => return Err(Error::runtime(format!("to_map needs (key, value) pairs, found {}", describe(&v)), span)),
                        }
                    }
                    Ok(Value::Map(Rc::new(m)))
                }
                _ => {
                    a.check(0, &["sep"], name, span)?;
                    let sep = match a.get(usize::MAX, "sep") {
                        Some(Value::Str(s)) => s.to_string(),
                        Some(Value::Char(c)) => (*c as char).to_string(),
                        None => String::new(),
                        Some(other) => return Err(Error::runtime(format!("sep must be a string, found {}", describe(other)), span)),
                    };
                    let parts: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                    Ok(Value::Str(parts.join(&sep).into()))
                }
            }
        }
        _ => Err(Error::runtime(format!("a chain has no method `{name}`"), span)),
    }
}

/// Sorted elements of a chain, for `to_sorted` and in-place `sort`.
/// `by` gives the sort key; `desc = true` sorts largest first. Stable.
pub fn sorted(it: &mut Interp, chain: &ChainV, a: &Args, method: &str, span: Span) -> Result<Vec<Value>> {
    a.check(0, &["by", "desc"], method, span)?;
    let by = match a.get(usize::MAX, "by") {
        Some(v @ Value::Lambda(_)) => Some(v.clone()),
        Some(other) => return Err(Error::runtime(format!("`by` needs a lambda, found {}", describe(other)), span)),
        None => None,
    };
    let desc = match a.get(usize::MAX, "desc") {
        Some(Value::Bool(b)) => *b,
        Some(other) => return Err(Error::runtime(format!("`desc` must be true or false, found {}", describe(other)), span)),
        None => false,
    };
    let mut keyed = Vec::new();
    it.drive(chain, &mut |s, v| {
        let key = match &by {
            Some(f) => s.call_value(f, vec![v.clone()], span)?,
            None => v.clone(),
        };
        keyed.push((key, v));
        Ok(true)
    })?;
    let mut err = None;
    keyed.sort_by(|(x, _), (y, _)| {
        let o = x.compare(y).unwrap_or_else(|| {
            err.get_or_insert_with(|| Error::runtime(format!("can't compare {} with {}", describe(x), describe(y)), span));
            std::cmp::Ordering::Equal
        });
        if desc { o.reverse() } else { o }
    });
    match err {
        Some(e) => Err(e),
        None => Ok(keyed.into_iter().map(|(_, v)| v).collect()),
    }
}
