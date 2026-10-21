# eggnog++ — plan

A small, fast, Rust-flavored language for solving Advent of Code puzzles.

**Goals:** learn how languages and compilers work (primary); use it for Advent of Code in December (stretch).

Keep this file current, so a fresh context or a sub-agent can pick up from here. Settled language decisions live in `SPEC.md`, not here.

## Decisions

- **Compiler written in Rust.** Self-hosting is an optional later milestone.
- **Output:** a native macOS ARM64 executable. `eggnog build day12.egg` produces `./day12`, with no VM.
- **Backend:** Cranelift (pure Rust, ARM64 backend, easy via Cargo). LLVM later only if benchmarks call for it.
- **Syntax:** Rust-like keywords and types, Python-style indented blocks with `:`. No lifetimes or borrow checker. Structs and enums are not designed yet.
- **Memory:** value semantics (assignment copies) and no freeing: allocation is a pointer bump, and the OS reclaims memory at exit. If a puzzle runs out of memory, free values when they're overwritten or go out of scope (every value has exactly one owner, so no reference counting).
- **Standard library implemented in Rust:** `input` and its parsing methods, `Grid`, the collections and chain operations are Rust code. In an interpreter they are methods on `Value`; in the compiler they become a runtime library. Filip's `advent-of-code/src/utils/grid.rs` is a starting point for `Grid`, after fixing its two known bugs (width with a trailing newline, no per-axis bounds check).

## Steps

1. **Design on paper.** *(Done: SPEC.md.)*
1b. **Solve varied AoC days in eggnog++** *(current)*. Filip writes solutions in `aoc-implementation/`, and they run on the reference interpreter. Claude checks each one against SPEC.md and challenges the design. Whatever turns out awkward gets decided into SPEC.md. This replaces the old "test on past AoC days" step: the language is tested on real puzzles while it's still cheap to change.
2. **Interpreter in Rust** (`interpreter/`, written by Filip). Lexer, parser, AST, tree-walking evaluator, standard library in Rust. Done when it passes every `aoc-implementation/` test. Also serves as a reference for checking the compiler's output.
3. **Native compiler.** Static type checker, then Cranelift codegen to ARM64 object files, linked with the system linker. Done when it passes every `aoc-implementation/` test. Open question for then: how compiled code passes values to the Rust runtime library (Rust generics can't cross a C ABI, so either one uniform 8-byte value representation, or a fixed set of instantiations). Benchmark against Filip's Rust AoC solutions.

## How we work

- **Language design is collaborative.** Filip proposes syntax and semantics. Claude checks feasibility (lexer, parser, type checker, Cranelift codegen), points out ambiguities and costs, and pushes back. Claude does not write the spec or the AoC solutions alone.
- **Design principle:** elegance comes from focusing only on AoC. Each feature has to make AoC solutions shorter or clearer. Anything that exists only because "real languages have it" stays out.
- **Stdlib principle:** helpers that keep coming back belong in the standard library, not in user code, if the pattern appears in several puzzles, is general, and its name follows the SPEC naming rules.
- **When the spec changes,** update together: `SPEC.md`, `reference/` (with a golden test), `editors/vscode/stdlib.json` (for built-ins), and this file.
- **Reference code:** Filip's Rust AoC solutions are in `/Users/filip/Dev/advent-of-code` (2015–2025, `src/yYYYY/dayDD.rs`, helpers in `src/utils/`, inputs in `input/YYYY/dayDD/input.txt`). Use them to find recurring patterns and as answers to check against.

## Status

- **1b:** done so far: 2015 d1, 2020 d10 (`memo fun`), 2022 d1–d6, 2023 d1. All 18 part files pass on the real inputs. Suggested next: 2022 d10 (multi-line string answer), 2023 d7, 2023 d5, 2023 d14, 2025 d11 (a `memo fun` with its own cache per call).
- **Step 2:** Filip has started (`interpreter/`, see its `GUIDE.md` for the 11-slice roadmap). The lexer, parser and evaluator still have `todo!()`s.
- Toolchain: rustc/cargo 1.99.1, Xcode installed, arm64.

## Tooling

- `aoc-implementation/yYYYY/dayDD/part1.egg`, `part2.egg`: one file per part, each producing a single answer. Shared code is copied between the two parts for now. Each part ends with `test file("input.txt") == <answer>`, with answers checked against Filip's Rust solutions or a Python port. `input.txt` is copied from the Rust repo and gitignored (AoC asks not to publish inputs). Exception: 2022 d5's saved input has only the moves, so its `input.txt` was rebuilt from the stack drawing in the Rust solution's comment, then a blank line, then the moves. Run every test with `aoc-implementation/test.sh [-v] [y2022/day03]`.
- `reference/`: a **reference interpreter** written by Claude (crate `eggnog-ref`), so solutions can be run while designing. It's not Filip's interpreter. It's dynamically typed (types are parsed and ignored), and compile-time rules are checked at runtime or not at all (see `reference/README.md`). Usage: `eggnog-ref run file.egg input.txt`, `eggnog-ref test file.egg`. Golden tests are in `reference/tests/programs/` (`cargo test`). Pipe `</dev/null` when running it non-interactively, or it waits for input on stdin.
- `editors/vscode/`: VS Code extension. Highlighting (TextMate grammar; it sets type colours for the "Visual Studio Dark/Light" themes, which have none), plus completions and hovers from `stdlib.json` (`extension.js`, no language server, not type-aware). `reference/tests/stdlib_catalog.rs` fails if the catalog lists something the reference interpreter doesn't implement. To install after changes: bump `version` in `package.json`, run `npx @vscode/vsce package --skip-license --allow-missing-repository` and `"/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code" --install-extension eggnog-lang-<version>.vsix --force`. A type-aware language server waits for the type checker (step 3).

## Open questions

- **Numeric edge cases:** shifts by ≥ 64 bits, and `float as int` when out of range or NaN. Claude proposed trapping for both.
- **`memo fun` with a grid or collection parameter:** allow it (slow to hash) or make it a compile error? Claude proposed a compile error.
- **User functions returning a maybe-value:** not allowed today, so 2023 d1's `digit_at` returns 0 as a "none" marker. If this keeps happening, let a `fun` return a maybe-value that callers must handle with `if x = f():`.
- **`map.get`:** probably a maybe-value, not designed yet.
- **`panic` and the never type:** `panic(...)` as the last expression of a function needs a bottom type to type-check.
- **`match`:** guards and destructuring patterns are not designed.
- **Lambdas:** multi-line lambda bodies (indentation inside parentheses).
- **Grid:** rotate/flip names; the `neighbors4/8` helper set isn't final. Puzzles that list "x,y" coordinates (2024 d18) must swap them to `(row, col)`.
- **`test` syntax details.**
- **Program structure:** if copying shared code between `part1.egg` and `part2.egg` gets cumbersome, consider Claude's alternative of `part1:` / `part2:` blocks after a shared prelude.

## Declined for now

- **Function references** (`.map(eval_round)`). Rules if it comes back: only directly as an argument (never stored), the parameter count must match, no `inout`, no default parameters at first, user functions only. Method references (`.map(\.int)`) don't help much, because most lambdas are method chains.
- **Regex:** `ints()`, `split_once` and list destructuring cover AoC's input formats.
- **`letters()`** (OCR for AoC's block-letter font).
- **Built-in `bfs` / `dijkstra`:** writing the algorithms is the point.
