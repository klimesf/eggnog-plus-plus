# eggnog-ref: reference interpreter

A runnable tree-walking interpreter for eggnog++, written to try the language
out while it's being designed. It is **not** the real implementation (that's
`../interpreter/`, written by hand); use it to run the AoC solutions, check
answers, and see how the language feels.

```sh
cargo build --release
alias egg="$PWD/target/release/eggnog-ref"

egg run day01.egg input.txt     # print the answer
egg test day01.egg              # run all `test` lines
egg run notes.md:2 input.txt    # run the 2nd ```egg block of a Markdown file
egg blocks notes.md             # list a Markdown file's blocks

../aoc-implementation/test.sh   # run the tests of every AoC solution
```

Input comes from the file argument, else piped stdin, else it's empty. The
trailing newline is stripped, as the spec says.

## How it differs from the spec

- **Dynamically typed.** Types in signatures are parsed and ignored, so type
  errors show up at runtime, when the code runs.
- **Runtime checks instead of compile-time ones.** Storing a chain, writing to
  a read-only parameter, `&` on a non-`inout` parameter and similar mistakes
  are runtime errors. The memo rule "don't change a captured variable after
  the `memo fun` is defined" is **not** checked.
- **No floats** yet.
- **Extras not in SPEC.md:** `print(…)` (to stderr), `min(a, b)`, `max(a, b)`,
  `abs(n)`, `count(pred)` / `count(value)`, `all(pred)`, `map.keys()`,
  `map.values()`, iterating a `Map` yields `(key, value)`.
- Iterating a `Set` or `Map` has no defined order.

## Layout

`lexer.rs` → `parser.rs` (AST in `ast.rs`) → `interp.rs` (statements,
expressions, places, calls, lazy chains) with the standard library in
`builtins.rs` and values in `value.rs`. Containers use `Rc` with
copy-on-write, which gives value semantics without copying on every read.

`tests/programs/*.egg` are golden tests (`cargo test`), and can be reused for
the hand-written interpreter.
