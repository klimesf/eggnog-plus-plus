# eggnog++ — plan

A small, fast, Rust-flavored language for solving Advent of Code puzzles.

**Goals:** learn how languages and compilers work (primary); use it for Advent of Code in December (stretch).

## Decisions so far

- **Compiler written in Rust.** Self-hosting is an optional later milestone. It needs a working compiler first.
- **Output:** a native macOS ARM64 executable. `eggnog build day12.egg` produces `./day12`: no runtime, no VM.
- **Backend:** Cranelift, a pure Rust code generator with an ARM64 backend. It's easy to set up via Cargo. An LLVM backend can be added later if benchmarks call for it.
- **Syntax:** like Rust, minus lifetimes and the borrow checker. `fn`, `let`, structs (no classes), enums with `match`, loops.
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

- **Language design is collaborative.** Filip proposes syntax and semantics; Claude checks whether each idea is feasible to implement (lexer, parser, type checker, Cranelift codegen), points out ambiguities and costs, and pushes back. Claude does not write the spec or the example programs alone.
- Settled decisions go into `SPEC.md` (to be created) as we agree on them. Open questions are tracked below.
- Keep this file current so a fresh context or a sub-agent can pick up from here.

## Status

- **Current step:** 1, design on paper. Nothing written yet.
- Toolchain checked (2026-10-18): rustc/cargo 1.99.1, Xcode installed, arm64.

## Open design questions

_(none recorded yet)_
