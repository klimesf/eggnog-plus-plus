# eggnog++ — plan

A small, fast, Rust-flavored language for solving Advent of Code puzzles.

**Goals:** learn how languages and compilers work (primary); use it for Advent of Code in December (stretch).

## Decisions so far

- **Compiler written in Rust.** Self-hosting is an optional later milestone. It needs a working compiler first.
- **Output:** a native macOS ARM64 executable. `eggnog build day12.egg` produces `./day12`, with no VM. The built-ins (see below) may be a Rust runtime library linked into the executable.
- **Backend:** Cranelift, a pure Rust code generator with an ARM64 backend. It's easy to set up via Cargo. An LLVM backend can be added later if benchmarks call for it.
- **Syntax:** Rust-like keywords and types, but Python-style indented blocks with `:` (see SPEC.md). No lifetimes or borrow checker. `fun`, `let`, loops; structs and enums are not designed yet.
- **Memory:** value semantics (assignment copies; no shared references) and no freeing. Allocation is a pointer bump, and the OS reclaims memory at exit. If a puzzle runs out of memory, free values when they're overwritten or go out of scope (every value has exactly one owner, so no reference counting is needed).
- **Built for AoC:** built-in lists, `Set`, `Map`, `Queue`, `Stack`, `MinQueue`, a 2D `Grid` with neighbor iteration, point arithmetic, and extracting all integers from a line (see SPEC.md).
- **Standard library implemented in Rust:** the built-ins (`input` and its parsing methods, `Grid`, the collections, chain operations) are Rust code, not eggnog++ code. In the interpreter they are methods on `Value`. Filip's `advent-of-code/src/utils/grid.rs` is a starting point for `Grid`, after fixing its two known bugs (width with a trailing newline, no per-axis bounds check). Keeping the heavy work in Rust is also what makes a tree-walking interpreter fast enough for AoC.

## Steps

1. **Design on paper.** Write a short language spec, and write BFS, DFS and Dijkstra in the imagined syntax to test the design before writing code. *(Done.)*
1b. **Solve varied AoC days on paper.** Filip writes solutions to past puzzles in eggnog++ (`examples/`), away from graphs and grids: parsing, strings, simulation, math, sorting. Claude checks each one against SPEC.md as a parser and evaluator would, reports what wouldn't parse or run, and challenges the design. The gaps found get decided into SPEC.md.
2. **Interpreter in Rust (the first target).** Lexer, parser, AST, tree-walking evaluator, with the standard library in Rust. Goal: run the examples and the 1b solutions. It also serves as a reference for checking the compiler's output.
3. **Native compiler.** Static type checker, then Cranelift codegen to ARM64 object files, linked with the system linker (Xcode command line tools). The Rust standard library from step 2 becomes a runtime library linked into the executable. Open question for then: how compiled code passes values to it (Rust generics can't cross a C ABI, so either one uniform 8-byte value representation, or a fixed set of instantiations). Benchmark against Filip's Rust AoC solutions.
4. **Test on past AoC days.** Solve puzzles from previous years with known answers and fix whatever turns out awkward.

## Prerequisites

- Rust toolchain (`rustup`)
- Xcode command line tools (`xcode-select --install`) for the linker

## How we work

- **Stdlib principle:** eggnog++ is AoC-focused, so helpers that keep coming back belong in the standard library (written in Rust), not in user code. Criteria: the pattern appears in several puzzles (check Filip's Rust repo), it's general rather than puzzle-specific, and its name follows the SPEC naming rules. Candidates are collected below under "Stdlib candidates".

- **Design principle:** elegance comes from focusing only on AoC. Each feature has to earn its place by making AoC solutions shorter or clearer. Anything that exists only because "real languages have it" stays out.
- **Reference code:** Filip's Rust AoC solutions are in `/Users/filip/Dev/advent-of-code` (2015–2025, `src/yYYYY/dayDD.rs`, helpers in `src/utils/`). Use them to see which patterns come up again and again, and as test programs with known answers.

- **Language design is collaborative.** Filip proposes syntax and semantics; Claude checks whether each idea is feasible to implement (lexer, parser, type checker, Cranelift codegen), points out ambiguities and costs, and pushes back. Claude does not write the spec or the example programs alone.
- Settled decisions go into `SPEC.md` (to be created) as we agree on them. Open questions are tracked below.
- Keep this file current so a fresh context or a sub-agent can pick up from here.

## Status

- **Step 1 (design on paper) is done** (2026-10-18). `SPEC.md` holds every decision. The examples are `examples/BFS.md` (flood fill, queue BFS, functional level-by-level BFS), `examples/Dijkstra.md` (MinQueue) and `examples/Recursion.md` (2023 d23 part 1: a backtracking DFS plus a memoized version that relies on the maze being acyclic). Iterative DFS = BFS with `Stack`, so it has no separate example.
- **Current step:** 1b, varied AoC days on paper (Filip asked for this before starting the interpreter). Reviewed so far: `examples/year2015_day01.md`, which led to `match` with colon arms, `if … then … else …`, input newline stripping, `chars()`/`count`/`enumerate`, and `test "inline"` vs `test file(...)`, `scan`/`position`, and from `examples/year2023_day01.md`: `is_digit`/`digit`, `find_last` on chains, triple-quoted strings; from d1 part 2: `else if`, `starts_with(p, at =)`, string slicing, list `first`/`last`, and the ASCII-only source rule (non-breaking spaces had slipped into three examples) (all in SPEC.md). **Program structure changed:** one file per part, each returning a single answer, because the part 2 example of 2023 d1 would make part 1 trap. Shared code is copied for now. Revisit (Claude's alternative: `part1:` / `part2:` blocks after a shared prelude) if copying gets cumbersome. Suggested next days: 2023 d1, 2022 d5, 2022 d10, 2023 d7, 2023 d5, 2023 d14.
- **Step 2 (interpreter), prepared but paused:** Filip writes the code, and Claude reviews, explains and challenges (Claude only did the boilerplate). The project is in `interpreter/` (crate `eggnog`, no dependencies):
  - `src/error.rs`: `Span` plus `Error` with `render` (source line and `^^^`). Done.
  - `src/token.rs`, `src/ast.rs`, `src/eval.rs`: minimal types for slice 1, to be grown by Filip.
  - `src/lexer.rs`: helpers done, `run()` is `todo!()`. Unit tests describe slice 1 plus INDENT/DEDENT.
  - `src/parser.rs`, `eval::run`: `todo!()`.
  - `src/main.rs`: CLI `eggnog run|tokens|ast <file> [input]`. Done.
  - `tests/programs.rs`: golden tests, `tests/programs/NAME.egg` + `NAME.expected` (+ optional `NAME.input`; `error: …` for expected failures).
  - `GUIDE.md`: pipeline overview, the 11-slice roadmap, and a step-by-step for slice 1 (lexer with INDENT/DEDENT, precedence climbing, checked arithmetic).
  - The slice-1 tests were validated against a throwaway reference implementation, which was not committed.
  - **Progress:** slice 1 not started. Not committed yet at the time of writing.
- Step 1 leftovers, to settle when the interpreter hits them: shifts by ≥ 64 bits and out-of-range `float as int` (Claude proposed trapping), `memo fun` with collection parameters (Claude proposed a compile error), grid rotate/flip names, the `test` syntax details.
- Toolchain checked (2026-10-18): rustc/cargo 1.99.1, Xcode installed, arm64.

## Tooling

- `editors/vscode/`: a VS Code extension that only highlights syntax (TextMate grammar). It covers `.egg` files and ```` ```egg ```` fenced blocks in Markdown. It's installed by symlink: `~/.vscode/extensions/local.eggnog-lang-0.0.1` → `editors/vscode`. Update the keyword lists in `syntaxes/eggnog.tmLanguage.json` as the syntax changes. Comments are `#`.

- **Observation (2023 d1):** user functions can't return a maybe-value (Option stays hidden), so `digit_at` returns 0 as a "none" marker. Watch whether this keeps happening. If so, consider letting a `fun` return a maybe-value that callers must handle with `if x = f():`.

- **From 2022 d5 (examples/year2022_day05.md v1), all 8 accepted into SPEC.md:** `s.split_once(sep)` → a pair of strings (traps if `sep` is missing); `[x] * n` to make a list of n copies (safe under value semantics, unlike Python); slicing lists `xs[a..b]`; `rev()` on lists and ranges (reversible sources only); list destructuring `let [a, b, c] = xs` (traps on a length mismatch); `c.is_letter()`; `Stack.top()` (a maybe-value); `to_string()` on a chain of chars; **in-place mutation through indexing** (`xs[i].push(x)` changes the element: place expressions, important for the interpreter); `Regex` declined for now, because `ints()` covers the AoC parsing cases. Also `Stack.pop_many(n)` / `push_all(xs)`, with a naming-rule exception for mutating verbs that return removed elements (2022 d5 parts differ only by `.rev()`). Before 2022 d10: multi-line string answers, `to_string(sep = …)` on chains (chosen over `join`), `grid.to_string()`. `letters()` (OCR for AoC's block font) was set aside for now. Claude's rewrite was checked with a Python port: `CMZ` on the public example, and `WCZTHTMPS` on Filip's input (moves file plus the stacks from the Rust comment), matching a port of the Rust part_a.

## Stdlib candidates

Helpers seen in solutions that could move into the stdlib. Add to this list during 1b. Promote one to SPEC.md once it has shown up in a few puzzles.

- _(Promoted straight to SPEC: `s.digits()` and `n.digits()`, because `to_digit` / `% 10` digit extraction appears in ~14 of Filip's Rust days: 2017 d1, 2019 d4/d8/d16, 2024 d9/d11, 2025 d3, …)_
- _(Already in SPEC: `ints()`, `to_char_grid`/`to_digit_grid`, `neighbors4/8`, `dir()`, `digit()`, `scan`, `position`, `find_last`.)_

## Open design questions

- ~~Generics~~: decided. Only the built-in containers are generic (see SPEC.md).
- ~~Numeric types~~: decided. `int` (i64) and `float` (f64), no implicit conversions, overflow always traps (see SPEC.md).
- **Edge cases still open:** shifts by >= 64 bits (trap or mask?), and `float as int` when the value is out of range or NaN (trap or saturate?).
- ~~Chars~~: decided. One ASCII byte (see SPEC.md).
- **Char arithmetic:** assumed for now that `char - char` gives `int` and `char + int` gives `char`. Other exceptions to the no-implicit-conversion rule get decided case by case.
- ~~Grids~~: decided. A built-in 2D `Grid<T>` (see SPEC.md). 3D/4D and unbounded grids use `Set`/`Map` keyed by tuples.
- ~~Grid details~~: decided (see SPEC.md). Coordinates are `(row, col)`. Watch out for puzzles that list "x,y" coordinates (2024 d18): swap them to `(y, x)`.
- **Option / missing values:** Filip doesn't want Option everywhere and will decide later. Something has to answer what `map.get(missing)`, `g.get(outside)` and `heap.pop()` on an empty heap do. Alternatives to Option: trap and check with `has` first, take a default argument, or give maps a default value.

### From the BFS draft (examples/BFS.md v1)

- ~~Blocks~~: decided. Python-style indentation with `:` (see SPEC.md).
- ~~Bindings~~: decided. `let` (placeholder name), everything is mutable (see SPEC.md).
- ~~Type inference~~: decided. Named functions require parameter and return types. `let`, lambdas and container element types are inferred within a function body (unification), see SPEC.md.
- ~~Conditional binding~~: decided. It replaces Option. Option stays hidden as much as possible (see SPEC.md).
- ~~Program structure~~: decided. Script form, where the file is the body of an implicit `solve(input)` (see SPEC.md). `test` syntax is proposed.
- **Directions:** lowercase `up`/`down`/`left`/`right` as built-ins take common variable names. Alternatives: `UP`…, `N/E/S/W`, or a `DIRS4` you can iterate over. Turning (`d.right()`) comes up often in AoC. BFS v2 uses `Dirs4` (PascalCase constant). The naming convention for constants is not settled.
- ~~Grid element types~~: decided. `input.to_char_grid()` / `input.to_digit_grid()` (see SPEC.md). Number tables are rows of `line.ints()`, not grids.
- **Naming:** `val queue = queue()` shadows the constructor. Capitalized types (`Queue()`, `Set()`) or `input.grid()` methods instead of free functions like `gridify`?
- ~~`fun` vs `fn`~~: decided. `fun`.
- ~~`match`~~: decided, with colon arms (see SPEC.md). Guards and destructuring are still open.
- **`panic` and the never type:** `panic("...")` at the end of `solve` needs a bottom type so the function still type-checks as returning `int`.
- ~~Direction names~~: decided (see SPEC.md).
- ~~Lambdas~~: decided. Non-escaping lambdas, `p => …` (see SPEC.md). Still open: multi-line lambda bodies (indentation inside parentheses), and whether to add built-in search functions (`bfs`, `dijkstra`). Leaning no, since writing the algorithms is the point.
- ~~Lazy chains~~: decided, with explicit `to_list()` / `to_set()` / `to_map()` (see SPEC.md). `grid.neighbors4(p)` yields only in-bounds neighbors (helper set not final).
- ~~Method naming~~: decided (see SPEC.md). `input.lines()` is lazy, and `find`/`filter`/`map` are exceptions.
- ~~Parameter passing~~: decided. Read-only by pointer, with explicit `inout` / `&` (see SPEC.md). Freeing memory: keep "never free" for now. Later the compiler can free on overwrite or scope exit, because every value has exactly one owner. Copies become moves on last use (an optimization).
- ~~Memoization~~: decided. `memo fun` (see SPEC.md). Still open: should a grid or collection parameter be allowed (slow to hash), or be a compile error?
- ~~Local functions / memo closures~~: decided (see SPEC.md). The memo cache is per run when the function captures, and captured variables can't change after definition.

### From the Dijkstra draft (examples/Dijkstra.md v1)

- ~~Priority queue~~: decided. `MinQueue` with `push(item, priority)` and `pop()` returning `(item, priority)` (see SPEC.md). Dijkstra v2 has the same shape as the BFS loop.
- ~~Missing keys~~: decided. `Map(default = v)`, and `Map()` traps (see SPEC.md). Named arguments are now general (see SPEC.md). Default values decided (constant expressions only).
- ~~Constant naming~~: decided. PascalCase (`Dirs4`, `int::Max`).
- ~~Tabs~~: decided. A lexer error (see SPEC.md). The VS Code extension forces spaces in `.egg` files.
- ~~Constructors~~: decided. The arguments are the same as one push/add (see SPEC.md).

### From the Recursion draft (examples/Recursion.md v1)

- Bugs in the draft: line 12 checks `'.'` instead of `'#'` (it would return 0 immediately), a `neighbords` typo on line 22, a tab on line 34, and a missing final `dfs(start, 0)` (the script produces no answer).
- Fits the rules: a local `fun` capturing and changing `seen`, recursion through a closure, a lambda calling the local fun inside a lazy chain (fully consumed by `.max()` before `seen.remove`), `let neighbors = []` with the element type inferred.
- ~~Grid rows / size~~: decided. `grid.find_last(c)` (see SPEC.md). Comments are `#` (used in Recursion v2).
- ~~Empty `.max()`~~: decided. A maybe-value (see SPEC.md).
- ~~Operator precedence~~: decided (see SPEC.md).
- ~~Char to direction~~: decided. `c.dir()`. The `to_` = allocation rule stays.
- ~~Day 23 alternative~~: added to Recursion.md. Line continuation and point `-` are now in SPEC.md. with slopes, the part 1 maze is a DAG. `memo fun longest(pos, prev)` with "don't step back to prev, never step onto a slope against its direction" needs no `seen` or backtracking. Verified with a Python port on Filip's input: 94 / 2186, 9 413 memo states, no dead ends.
