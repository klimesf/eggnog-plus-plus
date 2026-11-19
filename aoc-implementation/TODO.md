# TODO

From the 2026-11-06 review against ELEGANCE.md and LEARNINGS.md.

- [ ] **2023 d11 part 2:** pass the expansion factor as a parameter, so a test can check the puzzle's factors 10 (1030) and 100 (8410).
- [ ] **2023 d24 part 2** (from the 2026-11-11 review):
  - [ ] `algebra.egg`: one recursive `det()` instead of `det2`, `det3` and `det4` (base case, a sign by column index, a chain over the first row).
  - [ ] `algebra.egg`: drop the 6-argument `cramer` for `det()` plus a `with_column(matrix, col, values)` helper, so each unknown is one line: `let vx = det(with_column(left_side, 2, right_side)) / d`. Use it for the 2×2 systems of X, Y and Z too.
  - [ ] `part2.egg`: build the Z rows with a loop (or a small `fun`) like the 4×4 rows, instead of two hand-written rows with `xk…vzk`.

## 2024 (from the 2026-11-17 to 2026-11-19 reviews)

- [ ] **d3 (both parts):** rewrite as a lexer plus an evaluator, as practice for the interpreter.
  - [ ] Lexer: tokens `Mul`, `LParen`, `Int(n)`, `Comma`, `RParen`, `Do`, `Dont` and `Other`. Look for `mul` at every position (don't read the longest word, or `do_not_mul(5,5)` is lost), emit `Other` for characters that fit nothing, and try `don't` before `do`.
  - [ ] Parser: exactly `Mul LParen Int Comma Int RParen` in a row, each `Int` with 1 to 3 digits.
  - [ ] Evaluator: a loop over the calls with an `enabled` flag; part 1 ignores `Do` and `Dont`.
  - [ ] Add the test `mul(mul(2,3)` == 6: the current version consumes the character that ends a failed `mul(` and returns 0.
- [ ] **d6 part 2** (30 s):
  - [ ] Remember `(pos, dir)` only where the guard turns (measured: 20 s).
  - [ ] Start each search right before the new obstacle: while walking the original path, record where the guard stood and which way it faced just before entering each cell for the first time (measured with both changes: 6.4 s).
  - [ ] Elegance: `let next = position + Dirs4[direction]` at the top of the loop instead of three `Dirs4[direction]`.
  - [ ] Hint: the guard moves in straight lines until it hits something. How many steps does each straight run need if you know, for every row and column, where the obstacles are? The one new obstacle is the only thing that changes in each search.
- [ ] **d7 part 2** (7.7 s): hint: think about the last operator. Working forwards, all three always apply. Starting from the test value, when can the last one have been `*`? And when `||`? (measured: 60 ms)

## Built-ins (rule 3)

- [x] `.filter(p).count()` → `.count(p)`: 2022 d4 (both parts), 2023 d6 part 1, 2023 d10 part 2, 2023 d11 (both parts).
- [x] 2022 d6 (both parts): `enumerate().filter().map().first()` → `.position(…)`.
- [x] 2023 d1 part 2: the `ans +=` loop over lines → `.map(…).sum()`.

## Delete what does nothing (rule 8)

- [x] 2023 d17 (both parts): the `panic` after the search.
- [x] 2023 d19 part 1: `panic("no rule applies")`; the last rule always applies.
- [x] 2022 d2 (both parts): `_: panic(…)`; a `match` with no matching arm already stops.

## Shapes (rules 14 and 15)

- [ ] 2023 d19 part 1: the `("_", 0, 0, rule)` placeholder tuple → an enum, as in part 2.
- [ ] 2022 d11 (both parts): the parallel `monkeys[i]`, `items[i]`, `inspections[i]` → a `Monkey` struct.
- [ ] 2022 d11 part 2: `let lcm = 1` hides the built-in `lcm`; using it removes the "always a prime" assumption.

## Each case once, names (rules 2 and 5)

- [ ] 2023 d2 part 2: three identical `match` arms → one `Map(default = 0)` of the fewest cubes per color.
- [x] 2023 d6 part 2: `let dist = distance…` → keep the name `distance`, like `time`.

## Consistency and formatting

- [x] `.map(i => arrangements(i))` (2020 d10 part 2) and `.map(c => priority(c))` (2022 d3 part 1) → `(it)`.
- [x] 2023 d15 part 2: indent the `hash` chain; trailing whitespace after it.
- [x] 2023 d5 part 2: trailing whitespace after `apply_map`.
- [x] 2022 d12 part 2: 6-space indent in the `for` body.
