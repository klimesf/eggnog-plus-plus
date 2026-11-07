# TODO

From the 2026-11-06 review against ELEGANCE.md and LEARNINGS.md.

- [ ] **2023 d11 part 2:** pass the expansion factor as a parameter, so a test can check the puzzle's factors 10 (1030) and 100 (8410).

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
