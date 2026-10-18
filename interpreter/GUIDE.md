# Writing the eggnog++ interpreter: a guide

## The big picture

An interpreter is a pipeline of three stages. Each stage turns one data
structure into the next:

```
"1 + 2 * 3\n"                                   source text   (String)
   │ lexer.rs
   ▼
[Int(1) Plus Int(2) Star Int(3) Newline Eof]    tokens        (Vec<Token>)
   │ parser.rs
   ▼
      Binary(+)                                 AST           (ast::Program)
      /       \
  Int(1)    Binary(*)
            /      \
        Int(2)    Int(3)
   │ eval.rs
   ▼
Int(7)                                          value         (eval::Value)
```

- **The lexer** only cares about characters: where does a number end, is
  `==` one token or two, how deep is this line indented. It knows nothing
  about grammar.
- **The parser** only cares about structure: `*` binds tighter than `+`, a
  `while` has a condition and a block. It knows nothing about values.
- **The evaluator** walks the tree and computes things. It never sees
  characters or tokens.

Keeping these jobs separate is what makes an interpreter manageable.

## Work in vertical slices

Don't write the whole lexer, then the whole parser, then the whole
evaluator. Instead, take **one tiny feature all the way through the three
stages**, see it run, then grow every stage a little. Each slice ends with a
new program in `tests/programs/` that passes.

| # | Slice | What you learn |
|---|-------|----------------|
| 1 | Integer arithmetic: `1 + 2 * (30 - 4)` | the whole pipeline, precedence, overflow traps |
| 2 | `let`, variables, assignment, `+=` | environments (where variables live) |
| 3 | Booleans, comparisons, `and`/`or`/`!`, `if`/`else`, `while` | INDENT/DEDENT blocks, short-circuiting |
| 4 | `break`/`continue`, top-level `return` | non-local control flow |
| 5 | Strings, chars, `input`, `for` over ranges and lines | the first real AoC day (try 2022 d1) |
| 6 | `fun`, calls, recursion, named arguments, defaults | call frames, closures over the script |
| 7 | Tuples, lists, `Set`, `Map(default = …)`, `Queue`, `Stack`, `MinQueue` | value semantics, methods on built-ins |
| 8 | `Grid`, points, `Dirs4`, `c.dir()` | run `examples/BFS.md` |
| 9 | Lambdas and lazy chains (`map`, `filter`, `flat_map`, `max`, `to_list`) | iterators |
| 10 | Conditional binding (`while x = q.pop():`), maybe-values | run `examples/Dijkstra.md` |
| 11 | `memo fun`, `inout`, `test` blocks | run `examples/Recursion.md` |

The scaffold is set up for slice 1: `TokenKind`, `ExprKind` and `Value` hold
only what it needs, and the tests in `lexer.rs` and `tests/programs/`
describe it.

## Tools you have

```sh
cargo test                                   # unit tests + golden programs
cargo run -- tokens tests/programs/arithmetic.egg
cargo run -- ast    tests/programs/arithmetic.egg
cargo run -- run    tests/programs/arithmetic.egg [input.txt]
```

`tokens` and `ast` let you check each stage on its own. When something is
wrong, find the first stage whose output is wrong.

Errors carry a `Span` (byte range), and `Error::render` prints the source
line with a `^^^` underline. Give every token and AST node a span from the
start. It costs little now and is painful to add later.

## Slice 1, step by step

### 1. The lexer (`lexer.rs`)

The core is one loop: look at the next byte, decide what kind of token
starts there, consume it, push it.

```
loop:
    remember start = pos
    match peek():
        None            → finish (see below), stop
        ' '             → advance, skip it
        '#'             → skip to the end of the line
        '\n'            → handle the end of a line (see below)
        '0'..='9'       → consume all digits, parse the text as i64
                          (parse failure → "integer too large" error)
        '+' '-' '*' '/' '(' ')' → advance, push the matching token
        anything else   → error "unexpected character"
```

Run `cargo test lexer` until `arithmetic` and `spans_point_at_the_source`
pass. Then do indentation.

**Indentation: INDENT and DEDENT.** The parser should never count spaces.
The lexer turns indentation into tokens, the same way Python does it:

- Keep a stack of indentation widths, starting as `[0]`.
- At the **start of each line**, count the spaces. A tab is an error (SPEC).
- If the line is **blank or only a comment**, ignore it completely.
- Otherwise compare the width with the top of the stack:
  - **greater:** push it and emit `Indent`.
  - **equal:** emit nothing.
  - **smaller:** pop and emit `Dedent` until the top equals the width.
    If you pop past it (the width isn't one of the outer levels), that's
    an error: "dedent doesn't match any outer block".
- At the **end of a line** that had tokens, emit `Newline`.
- At the **end of the file**, emit `Newline` if the last line had tokens
  but no `\n`, then one `Dedent` for every width still above 0 on the stack,
  then `Eof`.

So for blocks, `Indent` works like `{` and `Dedent` like `}`. The tests
`indent_and_dedent` and its neighbours pin this down. Slice 1 has no blocks
yet, but the indentation logic is self-contained, so it's easier to finish
it now.

Tip: track "am I at the start of a line?" with a bool field on `Lexer`.
When it's true, measure the indentation before lexing anything else.

### 2. The AST (`ast.rs`)

Add what slice 1 needs to `ExprKind`:

```rust
Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
Unary  { op: UnOp,  operand: Box<Expr> },
```

with small `enum BinOp { Add, Sub, Mul, Div }` and `enum UnOp { Neg }`.
`Box` is needed because an `Expr` contains `Expr`s. Parentheses don't need
a node: `(1 + 2)` parses straight into the `Binary` inside.

### 3. The parser (`parser.rs`)

Give the parser a `Vec<Token>` and a position, with helpers like the
lexer's: `peek()`, `advance()`, and `expect(kind)`, which consumes a token
or returns an error such as "expected ')'".

**Statements** use plain *recursive descent*: one function per grammar rule.
For now a program is a list of expression statements, each followed by
`Newline`, until `Eof`.

**Expressions** need operator precedence. Use *precedence climbing*
(also called Pratt parsing). Each binary operator gets a binding power:
`+ -` → 1, `* /` → 2. Then:

```
parse_expr(min_power):
    left = parse_prefix()          # a number, '(' expr ')', or '-' operand
    loop:
        op = peek()
        if op isn't a binary operator, or power(op) < min_power: stop
        advance()
        right = parse_expr(power(op) + 1)    # +1 makes it left-associative
        left = Binary(op, left, right)
    return left
```

Start with `parse_expr(0)`. Trace `1 + 2 * 3` by hand once: the `*` binds
inside the recursive call for the right side of `+`, so you get
`1 + (2 * 3)`. And `10 - 3 - 2` becomes `(10 - 3) - 2` because of the
`+ 1`. Later, comparisons, `in`, `and` and `or` just become more rows in
the power table (SPEC.md has the order).

`parse_prefix` handles unary minus: consume `-` and parse the operand with
a power higher than `*`, so that `-2 * 3` is `(-2) * 3`.

Check your output with `cargo run -- ast …`.

### 4. The evaluator (`eval.rs`)

A recursive function `eval_expr(&Expr) -> Result<Value, Error>`:

- `Int(n)` → `Value::Int(n)`.
- `Binary` → evaluate left, evaluate right, then apply the operator.
  Overflow must trap (SPEC): use `i64::checked_add` / `checked_sub` /
  `checked_mul` / `checked_div`, and turn `None` into
  `Error::runtime("integer overflow", expr.span)`. Check for division by
  zero separately, so you can give the better message.
- `run` evaluates the statements in order and returns the last value.

When `cargo test` is all green, slice 1 is done. 🎉

## Hints for later slices

- **Environments (slice 2).** A variable lookup goes from a name to its
  value. Start with a `Vec<HashMap<String, Value>>` used as a stack of
  scopes: push one when a block starts, pop it when it ends, and look names
  up from the innermost scope outward.
- **Control flow (slice 4).** `break`, `continue` and `return` have to
  unwind through several Rust calls. A clean way is to have statement
  execution return `Result<Flow, Error>` with
  `enum Flow { Normal, Break, Continue, Return(Value) }`, and have loops
  react to it.
- **Value semantics (slice 7).** Rust's `Clone` gives you copies for free:
  assignment clones. Optimise only when a program is actually slow.
- **Lazy chains (slice 9).** Rust iterators are lazy too. Evaluating
  `xs.map(f).filter(g)` can build a boxed Rust iterator whose closures call
  back into the evaluator. Consuming it (`max`, `to_list`, a `for` loop)
  drives it, which gives the element-by-element order SPEC.md requires.
- **Traps.** Every runtime error should point at a span. When you add
  `panic(...)` and failed lookups, reuse `Error::runtime`.

## Reading

- *Crafting Interpreters* by Robert Nystrom (free at craftinginterpreters.com),
  chapters 4–10. It builds a tree-walking interpreter step by step, the
  same architecture as this one (in Java, but the ideas map 1:1).
- "Simple but Powerful Pratt Parsing" by Alex Kladov (matklad's blog).
  Precedence climbing in Rust, short and very clear.
- The Python reference, "Lexical analysis → Indentation", for the
  INDENT/DEDENT algorithm you are implementing.
