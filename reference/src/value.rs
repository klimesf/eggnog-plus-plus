//! Runtime values.
//!
//! Containers live behind `Rc` and are copied on write (`Rc::make_mut`), which
//! gives value semantics (assignment copies) without copying on every read.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::ast::{FunDecl, Lambda, Name};

pub type Env = Rc<RefCell<Scope>>;

#[derive(Default)]
pub struct Scope {
    pub vars: HashMap<Name, Value>,
    pub parent: Option<Env>,
    /// Parameters that may not be changed (everything that isn't `inout`).
    pub readonly: HashSet<Name>,
}

pub fn new_scope(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(Scope { vars: HashMap::new(), parent, readonly: HashSet::new() }))
}

#[derive(Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Bool(bool),
    Char(u8),
    Str(Rc<str>),
    Tuple(Rc<Vec<Value>>),
    List(Rc<Vec<Value>>),
    Set(Rc<HashSet<Value>>),
    Map(Rc<MapV>),
    Queue(Rc<VecDeque<Value>>),
    Stack(Rc<Vec<Value>>),
    MinQueue(Rc<MinQ>),
    Grid(Rc<GridV>),
    Range { start: Option<i64>, end: Option<i64> },
    Chain(Rc<ChainV>),
    Func(Rc<Func>),
    Lambda(Rc<LambdaV>),
    /// The "no value" result of a maybe-operation (`pop()` on an empty
    /// queue, `find` with no match, …). Only a conditional binding may see it.
    Missing,
}

#[derive(Clone, Default)]
pub struct MapV {
    pub map: HashMap<Value, Value>,
    pub default: Option<Value>,
}

#[derive(Clone, Default)]
pub struct MinQ {
    pub heap: BinaryHeap<HeapEntry>,
    pub seq: u64,
}

#[derive(Clone)]
pub struct HeapEntry {
    pub prio: i64,
    pub seq: u64,
    pub item: Value,
}

impl PartialEq for HeapEntry {
    fn eq(&self, o: &Self) -> bool {
        self.prio == o.prio && self.seq == o.seq
    }
}
impl Eq for HeapEntry {}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for HeapEntry {
    /// Reversed, so Rust's max-heap pops the lowest priority first (FIFO among equals).
    fn cmp(&self, o: &Self) -> Ordering {
        o.prio.cmp(&self.prio).then(o.seq.cmp(&self.seq))
    }
}

#[derive(Clone)]
pub struct GridV {
    pub rows: i64,
    pub cols: i64,
    pub cells: Vec<Value>,
}

impl GridV {
    pub fn contains(&self, r: i64, c: i64) -> bool {
        r >= 0 && r < self.rows && c >= 0 && c < self.cols
    }

    pub fn idx(&self, r: i64, c: i64) -> usize {
        (r * self.cols + c) as usize
    }
}

/// A lazy chain: a source plus stages, driven element by element.
pub struct ChainV {
    pub source: Source,
    pub stages: Vec<StageOp>,
    /// Where the chain was built, for error messages.
    pub span: crate::error::Span,
}

#[derive(Clone)]
pub enum Source {
    Values { items: Rc<Vec<Value>>, rev: bool },
    Chars { s: Rc<str>, rev: bool },
    /// `end` is exclusive; `None` means no end.
    Range { start: i64, end: Option<i64>, rev: bool },
}

#[derive(Clone)]
pub enum StageOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Enumerate(i64),
    Scan(Value, Value),
    Take(i64),
    Skip(i64),
    /// Keeps the elements that are `in` the collection.
    Intersect(Value),
    Chunks(usize),
    Windows(usize),
    /// Pairs each element with the next one of this list (the collected argument).
    Zip(Rc<Vec<Value>>),
}

pub struct Func {
    pub decl: Rc<FunDecl>,
    pub env: Env,
    pub cache: RefCell<HashMap<Vec<Value>, Value>>,
}

pub struct LambdaV {
    pub ast: Rc<Lambda>,
    pub env: Env,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Unit => "nothing",
            Value::Int(_) => "int",
            Value::Bool(_) => "bool",
            Value::Char(_) => "char",
            Value::Str(_) => "string",
            Value::Tuple(_) => "tuple",
            Value::List(_) => "list",
            Value::Set(_) => "Set",
            Value::Map(_) => "Map",
            Value::Queue(_) => "Queue",
            Value::Stack(_) => "Stack",
            Value::MinQueue(_) => "MinQueue",
            Value::Grid(_) => "Grid",
            Value::Range { .. } => "range",
            Value::Chain(_) => "chain",
            Value::Func(_) => "function",
            Value::Lambda(_) => "lambda",
            Value::Missing => "missing value",
        }
    }

    pub fn tuple(items: Vec<Value>) -> Value {
        Value::Tuple(Rc::new(items))
    }

    pub fn point(r: i64, c: i64) -> Value {
        Value::tuple(vec![Value::Int(r), Value::Int(c)])
    }

    pub fn as_point(&self) -> Option<(i64, i64)> {
        match self {
            Value::Tuple(t) if t.len() == 2 => match (&t[0], &t[1]) {
                (Value::Int(r), Value::Int(c)) => Some((*r, *c)),
                _ => None,
            },
            _ => None,
        }
    }

    /// How a value looks inside a collection (strings and chars quoted).
    pub fn repr(&self) -> String {
        match self {
            Value::Str(s) => format!("{s:?}"),
            Value::Char(c) => format!("{:?}", *c as char),
            _ => self.to_string(),
        }
    }

    /// Total order used by `<`, `min`, `max` and sorting.
    pub fn compare(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Char(a), Value::Char(b)) => Some(a.cmp(b)),
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
            (Value::Tuple(a), Value::Tuple(b)) | (Value::List(a), Value::List(b)) => {
                for (x, y) in a.iter().zip(b.iter()) {
                    match x.compare(y)? {
                        Ordering::Equal => {}
                        o => return Some(o),
                    }
                }
                Some(a.len().cmp(&b.len()))
            }
            _ => None,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Unit, Value::Unit) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) | (Value::List(a), Value::List(b)) | (Value::Stack(a), Value::Stack(b)) => a == b,
            (Value::Set(a), Value::Set(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a.map == b.map,
            (Value::Queue(a), Value::Queue(b)) => a == b,
            (Value::Grid(a), Value::Grid(b)) => a.rows == b.rows && a.cols == b.cols && a.cells == b.cells,
            (Value::Range { start: a, end: b }, Value::Range { start: c, end: d }) => a == c && b == d,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        std::mem::discriminant(self).hash(h);
        match self {
            Value::Int(n) => n.hash(h),
            Value::Bool(b) => b.hash(h),
            Value::Char(c) => c.hash(h),
            Value::Str(s) => s.hash(h),
            Value::Tuple(items) | Value::List(items) | Value::Stack(items) => items.hash(h),
            Value::Queue(items) => items.hash(h),
            Value::Grid(g) => {
                g.rows.hash(h);
                g.cols.hash(h);
                g.cells.hash(h);
            }
            _ => {}
        }
    }
}

fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Char(c) => write!(f, "{}", *c as char),
            Value::Str(s) => write!(f, "{s}"),
            Value::Tuple(t) => write!(f, "({})", join(t.iter().map(Value::repr))),
            Value::List(l) => write!(f, "[{}]", join(l.iter().map(Value::repr))),
            Value::Set(s) => write!(f, "Set({})", join(s.iter().map(Value::repr))),
            Value::Map(m) => write!(f, "Map({})", join(m.map.iter().map(|(k, v)| format!("{}: {}", k.repr(), v.repr())))),
            Value::Queue(q) => write!(f, "Queue({})", join(q.iter().map(Value::repr))),
            Value::Stack(s) => write!(f, "Stack({})", join(s.iter().map(Value::repr))),
            Value::MinQueue(q) => write!(f, "MinQueue({} items)", q.heap.len()),
            Value::Grid(g) => {
                for r in 0..g.rows {
                    if r > 0 {
                        writeln!(f)?;
                    }
                    for c in 0..g.cols {
                        write!(f, "{}", g.cells[g.idx(r, c)])?;
                    }
                }
                Ok(())
            }
            Value::Range { start, end } => {
                write!(f, "{}..{}", start.map(|n| n.to_string()).unwrap_or_default(), end.map(|n| n.to_string()).unwrap_or_default())
            }
            Value::Chain(_) => write!(f, "<chain>"),
            Value::Func(func) => write!(f, "<fun {}>", func.decl.name),
            Value::Lambda(_) => write!(f, "<lambda>"),
            Value::Missing => write!(f, "<missing>"),
        }
    }
}
