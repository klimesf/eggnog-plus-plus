# eggnog++ — plan

A small, fast, Rust-flavored language for solving Advent of Code puzzles.

**Goals:** learn how languages and compilers work (primary); use it for Advent of Code in December (stretch).

Keep this file current, so a fresh context or a sub-agent can pick up from here. Settled language decisions live in `SPEC.md`, not here.

## Decisions

- **Compiler written in Rust.** Self-hosting is an optional later milestone.
- **Output:** a native macOS ARM64 executable. `eggnog build day12.egg` produces `./day12`, with no VM.
- **Backend:** Cranelift (pure Rust, ARM64 backend, easy via Cargo). LLVM later only if benchmarks call for it.
- **Syntax:** Rust-like keywords and types, Python-style indented blocks with `:`. No lifetimes or borrow checker. No structs or enums so far (see "Declined for now").
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
- **Keep the language as small as possible.** Filip wants every addition challenged. Before adding a method, keyword or syntax, Claude shows how the same thing reads with what already exists. If the existing form is about as short and clear, the addition stays out, even when Filip proposed it. Example: `for_each` was declined because `for n in xs.filter(…): …` already fits on one line. Adding is cheap now and expensive later (spec, reference, Filip's interpreter, compiler, extension), and two ways to write the same thing make solutions harder to compare.
- **Stdlib principle:** helpers that keep coming back belong in the standard library, not in user code, if the pattern appears in several puzzles, is general, and its name follows the SPEC naming rules.
- **When the spec changes,** update together: `SPEC.md`, `reference/` (with a golden test), `editors/vscode/stdlib.json` (for built-ins), and this file.
- **Reference code:** Filip's Rust AoC solutions are in `/Users/filip/Dev/advent-of-code` (2015–2025, `src/yYYYY/dayDD.rs`, helpers in `src/utils/`, inputs in `input/YYYY/dayDD/input.txt`). Use them to find recurring patterns and as answers to check against.

## Status

- **1b:** done so far: 2015 d1, 2020 d10 (`memo fun`), 2022 d1–d12, 2023 d1–d11, d12 and d14 part 1 (53 part files). Recent additions driven by these: list/string comparison, strings need `.chars()` for chain methods, `gcd`/`lcm` (2023 d8; also 2019 d10/d12, 2022 d21, 2023 d20). Suggested next: 2023 d12 and d14 part 2, 2025 d11 (a `memo fun` with its own cache per call).
- **Step 2:** Filip has started (`interpreter/`, see its `GUIDE.md` for the 11-slice roadmap). The lexer, parser and evaluator still have `todo!()`s.
- Toolchain: rustc/cargo 1.99.1, Xcode installed, arm64.

## Tooling

- `aoc-implementation/yYYYY/dayDD/part1.egg`, `part2.egg`: one file per part, each producing a single answer. Shared code is copied between the two parts for now. Each part ends with `test file("input.txt") == <answer>`, with answers checked against Filip's Rust solutions or a Python port. `input.txt` is copied from the Rust repo and gitignored (AoC asks not to publish inputs). Exception: 2022 d5's saved input has only the moves, so its `input.txt` was rebuilt from the stack drawing in the Rust solution's comment, then a blank line, then the moves. Run every test with `aoc-implementation/test.sh [-v] [y2022/day03]`.
- `reference/`: a **reference interpreter** written by Claude (crate `eggnog-ref`), so solutions can be run while designing. It's not Filip's interpreter. It's dynamically typed, but checks each `fun`'s argument and return values against the declared types at runtime. Other compile-time rules are checked at runtime or not at all (see `reference/README.md`). Usage: `eggnog-ref run file.egg input.txt`, `eggnog-ref test file.egg`. Golden tests are in `reference/tests/programs/` (`cargo test`). Pipe `</dev/null` when running it non-interactively, or it waits for input on stdin.
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

## Declined for now

- **Function references** (`.map(eval_round)`). The implicit `it` mostly covers this: `.map(eval_round(it))`. Rules if it comes back: only directly as an argument (never stored), the parameter count must match, no `inout`, no default parameters at first, user functions only. Method references (`.map(\.int)`) don't help much, because most lambdas are method chains. Storing or returning a function (a monkey's operation in 2022 d11) is a bigger step: escaping closures need capture by value, heap-allocated environments, function types and indirect calls. Store the operation as data instead (the `"old * 19"` string) and interpret it with a plain `fun apply(operation, old)`.
- **Regex:** `ints()`, `split_once` and list destructuring cover AoC's input formats.
- **`letters()`** (OCR for AoC's block-letter font).
- **Built-in `bfs` / `dijkstra`:** writing the algorithms is the point.
- **`for_each` on chains:** `for x in chain.filter(…): …` already fits on one line, and a `for` body can grow to several statements.
- **Structs:** a struct holding only constants is a named tuple, and tuple destructuring names the fields where they're used: `for (i, (operation, div, if_true, if_false)) in monkeys.enumerate():` (2022 d11, with the changing `items` in their own list). Revisit when a puzzle has a record with many fields used in many places, or fields that change inside a collection (`monkeys[i].items.push(x)`).
- **List `+`:** append with `xs.push(x)`, join with `[xs, ys].flat_map(it)` (lazy). `acc = acc + [x]` would invite a quadratic copy each step, and `+` already means element-wise addition on tuples (`pos + Up`). No solution has needed it yet (2022 d10 was better without it). If joins keep coming up, consider `xs += ys` only, as an in-place append. The reference interpreter rejects list `+`.
- **Tuple indexing** (`pos[0]`, `pos.0`): destructuring names the parts, `let (dr, dc) = head - tail` (2022 d9). A runtime index can't be typed for mixed tuples like `(int, char)`, so `t[i]` would need a literal-only rule different from lists. If it comes back, use Rust's literal-only `t.0`. The reference interpreter rejects `t[i]`.
- **`part1:` / `part2:` blocks after a shared prelude:** copying the shared code between `part1.egg` and `part2.egg` is fine. Each part stays a complete program that can be read alone, and run time isn't a goal here (deduplicating mattered only for the Rust solutions' whole-year-in-1-second challenge).
- **`fold`:** `scan(init, f).last()` covers one-expression folds, and a `for` loop with an accumulator covers the rest (lambdas are single expressions). The common folds are already terminals (`sum`, `product`, `min`, `max`, `count`, `all`, `any`), and a folded collection would invite the quadratic copy-per-step.
- **`|=`:** there is no `|` operator yet, so it would be a one-off or-assign. `x = x or cond` and `if cond: x = true` are as short and short-circuit. Revisit together with bitwise operators on ints (`&`, `|`, `^`, `<<`, `>>`), which would bring `|=` along like `+=`.
- **`unique()`:** `xs.to_set().len() == n` already tests for distinct elements, and a lazy `unique` would allocate a hidden set.
- **`swap`:** a free `swap(&a, &b)` would need `inout` references to cells inside a collection, and two of them into the same grid alias each other. If it comes back, make it a method like Rust's: `grid.swap(p, q)`, `xs.swap(i, j)`. 2023 d14 didn't need it: moving a rock one step at a time is `grid[pos] = '.'` and `grid[pos + Up] = 'O'`, which says what happens and can't go wrong in either order, so `swap` would only save one line. In Filip's Rust solutions `.swap(` appears only in 2017 d10 and d16, both list swaps by index. Revisit when puzzles keep swapping cells that aren't "something into an empty cell".
