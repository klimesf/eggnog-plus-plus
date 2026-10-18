# eggnog++ — plan

A small, fast, Rust-flavored language for solving Advent of Code puzzles.

**Goals:** learn how languages and compilers work (primary); use it for Advent of Code in December (stretch).

## Decisions so far

- **Compiler written in Rust.** Self-hosting is an optional later milestone. It needs a working compiler first.
- **Output:** a native macOS ARM64 executable. `eggnog build day12.egg` produces `./day12`: no runtime, no VM.
- **Backend:** Cranelift, a pure Rust code generator with an ARM64 backend. It's easy to set up via Cargo. An LLVM backend can be added later if benchmarks call for it.
- **Syntax:** Rust-like keywords and types, but Python-style indented blocks with `:` (see SPEC.md). No lifetimes or borrow checker. `fn`, `let`, structs (no classes), enums with `match`, loops.
- **Memory:** value semantics (assignment copies; no shared references) and no freeing. Allocation is a pointer bump, and the OS reclaims memory at exit. Add reference counting only if a real puzzle runs out of memory.
- **Built for AoC:** built-in `Vec`, `HashMap`, deque and priority queue; a 2D grid type with neighbor iteration; point arithmetic (`p + (0, 1)`); extracting all integers from a line.
- **Standard library:** read a file, write to stdout. That's it.

## Steps

1. **Design on paper.** Write a short language spec, and write BFS, DFS and Dijkstra in the imagined syntax to test the design before writing code.
2. **Interpreter in Rust.** Lexer, parser, AST, tree-walking evaluator, enough to run the three example programs. It also serves as a reference for checking the compiler's output.
3. **Native compiler.** Static type checker, then Cranelift codegen to ARM64 object files, linked with the system linker (Xcode command line tools). Benchmark against your Rust AoC solutions.
4. **Test on past AoC days.** Solve puzzles from previous years with known answers and fix whatever turns out awkward.

## Prerequisites

- Rust toolchain (`rustup`)
- Xcode command line tools (`xcode-select --install`) for the linker

## How we work

- **Design principle:** elegance comes from focusing only on AoC. Each feature has to earn its place by making AoC solutions shorter or clearer. Anything that exists only because "real languages have it" stays out.
- **Reference code:** Filip's Rust AoC solutions are in `/Users/filip/Dev/advent-of-code` (2015–2025, `src/yYYYY/dayDD.rs`, helpers in `src/utils/`). Use them to see which patterns come up again and again, and as test programs with known answers.

- **Language design is collaborative.** Filip proposes syntax and semantics; Claude checks whether each idea is feasible to implement (lexer, parser, type checker, Cranelift codegen), points out ambiguities and costs, and pushes back. Claude does not write the spec or the example programs alone.
- Settled decisions go into `SPEC.md` (to be created) as we agree on them. Open questions are tracked below.
- Keep this file current so a fresh context or a sub-agent can pick up from here.

## Status

- **Step 1 (design on paper) is done** (2026-10-18). `SPEC.md` holds every decision. The examples are `examples/BFS.md` (flood fill, queue BFS, functional level-by-level BFS), `examples/Dijkstra.md` (MinQueue) and `examples/Recursion.md` (2023 d23 part 1: a backtracking DFS plus a memoized version that relies on the maze being acyclic). Iterative DFS = BFS with `Stack`, so it has no separate example.
- **Current step:** 2, the interpreter. Still open from step 1, to settle when the interpreter hits them: shifts by ≥ 64 bits and out-of-range `float as int` (Claude proposed trapping), `match` / if-expressions, `memo fun` with collection parameters (Claude proposed a compile error), grid rotate/flip names, the `test` syntax details.
- Toolchain checked (2026-10-18): rustc/cargo 1.99.1, Xcode installed, arm64.

## Tooling

- `editors/vscode/`: a VS Code extension that only highlights syntax (TextMate grammar). It covers `.egg` files and ```` ```egg ```` fenced blocks in Markdown. It's installed by symlink: `~/.vscode/extensions/local.eggnog-lang-0.0.1` → `editors/vscode`. Update the keyword lists in `syntaxes/eggnog.tmLanguage.json` as the syntax changes. Comments are `#`.

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
- **`match`:** removed from BFS for now and will come back later. When it does, Claude suggests value patterns with optional guards. `break`/`continue` inside an arm apply to the loop.
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
