//! Runtime checks of the types written in function signatures.
//!
//! The reference interpreter has no type checker, so it checks each argument
//! and return value against its declared type when a function is called.
//! Containers are checked by kind plus one sample element, which keeps the
//! check O(1) per value while still catching most mistakes.

use std::fmt;

use crate::ast::Type;
use crate::value::Value;

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Bool => write!(f, "bool"),
            Type::Char => write!(f, "char"),
            Type::Str => write!(f, "string"),
            Type::Float => write!(f, "float"),
            Type::List(t) => write!(f, "[{t}]"),
            Type::Tuple(ts) => {
                write!(f, "(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            Type::Container(name, args) => {
                write!(f, "{name}<")?;
                for (i, t) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ">")
            }
        }
    }
}

/// Does `v` fit `t`? Containers are checked by kind and one sample element.
pub fn fits(v: &Value, t: &Type) -> bool {
    let sample = |x: Option<&Value>, t: &Type| x.is_none_or(|x| fits(x, t));
    match (v, t) {
        (Value::Int(_), Type::Int) | (Value::Bool(_), Type::Bool) | (Value::Char(_), Type::Char) | (Value::Str(_), Type::Str) => true,
        (Value::List(xs), Type::List(e)) => sample(xs.first(), e),
        (Value::Tuple(xs), Type::Tuple(ts)) => xs.len() == ts.len() && xs.iter().zip(ts).all(|(x, t)| fits(x, t)),
        (Value::Set(s), Type::Container("Set", a)) => sample(s.iter().next(), &a[0]),
        (Value::Map(m), Type::Container("Map", a)) => {
            m.map.iter().next().is_none_or(|(k, v)| fits(k, &a[0]) && fits(v, &a[1]))
                && m.default.as_ref().is_none_or(|d| fits(d, &a[1]))
        }
        (Value::Queue(q), Type::Container("Queue", a)) => sample(q.front(), &a[0]),
        (Value::Stack(s), Type::Container("Stack", a)) => sample(s.last(), &a[0]),
        (Value::MinQueue(q), Type::Container("MinQueue", a)) => sample(q.heap.peek().map(|e| &e.item), &a[0]),
        (Value::Grid(g), Type::Container("Grid", a)) => sample(g.cells.first(), &a[0]),
        _ => false,
    }
}

/// Like `describe`, but names a list's element type, so a mismatch inside a
/// container is visible: "a list of string".
pub fn describe_typed(v: &Value) -> String {
    match v {
        Value::List(xs) if !xs.is_empty() => format!("a list of {}", xs[0].type_name()),
        _ => crate::interp::describe(v),
    }
}
