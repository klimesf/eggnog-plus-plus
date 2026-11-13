# Choosing the right tool

Lessons about picking algorithms for AoC puzzles, drawn from the solutions in `aoc-implementation/`. How to write the code elegantly is in `ELEGANCE.md`. New lessons are added as puzzles teach them.

## Check a technique's precondition before reaching for it

Every standard technique only works when the problem has a certain shape. Before using one, ask whether this puzzle has that shape:

| Technique | Works only when | Where it didn't hold |
|---|---|---|
| `memo fun` | the same call happens more than once | 2023 d12 part 1: every call got a different string, so the cache never hit |
| memoizing a result | the subproblems don't loop back on themselves | 2023 d16: beams cycle, and merged sets cost as much as walking |
| Dijkstra's visited set | a node is everything that decides the next moves | 2023 d17: the position alone wasn't enough |
| A* | the estimate is close to the real cost and never above it | 2023 d17: the estimate was too low to prune, so it ran slower |
| cycle detection | the state repeats, and the whole state is what you compare | (held) 2023 d14: the whole grid repeats after 126 cycles |

When the precondition fails, the technique either gives wrong answers (d17's visited set) or only adds code (d12's `memo`, d17's A*).

## A node is everything that decides what happens next

In a graph search, two arrivals are the same node only if every future move is the same from both. In 2023 d17 the allowed moves depend on the direction and on the steps already taken in it, so a node is (position, direction, steps), not the position. Get this wrong and Dijkstra gives wrong answers (pruning by position) or runs slowly (re-expanding states it has already handled).

## When it's slow, ask whether the growth or the constant is the problem

- **The growth is the problem** (exponential, like trying both values for every `?` in 2023 d12): no faster machine helps; the algorithm has to change.
- **The constant is the problem** (a linear walk done many times, like 2023 d14's tilts or d16's 440 beams): the algorithm is fine; it's the interpreter's speed. The compiler will fix it, so don't make the code worse for it.

Estimate the work before optimizing: number of states × work per state. 2023 d17 has about 240,000 states; at ~1 µs per statement in the reference interpreter, 2–3 s is simply that cost.

## Generate the candidates instead of filtering them all

When most of a range fails one cheap rule, build only the values that pass it, and keep the full definition as the filter. In 2019 d4 only about 3,000 of the 541,000 numbers have digits that never go down; a recursion where each digit starts from the previous one builds just those, and the run went from 1.29 s to 24 ms. Keep the full check (`meets`, written like the puzzle text) even though the generator already enforces part of it: the examples then test the rules directly, the repeated check is cheap on the few candidates, and the answer stays right as long as the generator never *misses* a valid value. Producing extra candidates would only cost time. In 2025 d2 the ranges hold 1.4 million IDs, but an invalid ID is one short part repeated, so building it from the part (`part * pow(10, d) + part`, repeated) went from 12 s to 4 ms. Start the part at the range's own first digits, not at the smallest part of that length, or the loop still walks through thousands of candidates below the range. A Set drops IDs that come from two part lengths (`222222` is `2` six times and `222` twice).

## Greedy is right when the first choice outweighs everything after it

To build the largest number from digits picked in order (2025 d3: 12 batteries out of 100), the first digit matters more than all later ones together, so take the largest digit you can, digit by digit, without searching. "Can" means it leaves enough digits after it for the rest (`start..len - left`), and among equal digits take the earliest, which leaves the most choice for later. A stable sort gives the earliest for free: `(start..end).to_sorted(by = batteries[it], desc = true).first()`. Part 1 (2 digits) is the same idea; writing the two picks out by hand reads fine there.

## When a change only affects its neighbors, recheck only the neighbors

Repeating "scan everything, apply every change, scan again" until nothing changes costs a full scan per round. In 2025 d4 part 2, removing a roll can only make its 8 neighbors removable, so keep a stack of positions to check: start with every position, and after each removal push its neighbors. The order doesn't change the result, because a removal only ever lowers other rolls' counts: anything removable stays removable until it's removed. That went from 1.6 s (rounds) to 233 ms. If a change could also make cells *stop* qualifying, the order would matter, and rounds would be the safe choice.

## Parse nested input with one call per value

Nested input (2022 d13 packets like `[1,[2,[3]]]`) parses most simply with a recursive function where each call reads exactly one value: a whole number, or a list whose loop calls the function once per element and stops at `]`. Pass the position as `inout` (`fun parse(packet: string, inout at: int) -> Data`), so each call moves it forward itself; returning `(value, position)` pairs makes every caller unpack and reassign. Check for `]` before reading an element, or `[]` gets parsed as a list holding an empty list. In d13, a loop condition `i < packet.len()` hid exactly that bug, and the tests still passed. Compared with a character loop that keeps a stack of open lists and a buffer for the digits, it's shorter, handles `[]` without a special case, and ran faster (34 ms vs 47 ms).

## Search one digit at a time when the output is built digit by digit

In 2024 d17 part 2, each loop of the program shifts A right by 3 and prints one number, so the output has one number per base-8 digit of A, and the last number sees only the top digit. Instead of trying every A, fix A's digits from the top: for each new digit try `0..8` and keep the ones whose output matches the end of the program. A digit that matches can still lead to a dead end, because the higher bits leak into later outputs, so it's a search with backtracking (a recursion), not a greedy pick; trying the digits in increasing order makes the first complete A the lowest. Check each candidate before recursing on it, not on entry, so the starting call needs no special case, and compare whole lists (`run(candidate) != program[pos - 1..]`), which also catches a wrong length.

## Make the equations linear, then solve the small unknowns first

2023 d24 part 2 (a rock that hits 300 hailstones) looks like it needs a solver: each hailstone adds its own collision time, and the time multiplies the rock's unknown velocity. Remove the time within one hailstone (multiply its x equation by the y factor and the other way round, without dividing, so a matching speed can't break it), and the products of two unknowns left over (`X·VY`, `Y·VX`) are the same for every hailstone, so subtracting two hailstones' equations cancels them. That leaves linear equations, one per pair. Before solving, estimate the sizes: Cramer's determinant for an unknown replaces its column with the right-hand side (here positions × velocities, about 10¹⁷). Replacing a column of small numbers (velocity differences) with it overflowed even 128 bits for X and Y; replacing a column of big ones fit for VX and VY. So solve the velocities first, then put them back and solve X, Y and Z as 2×2 systems with small numbers. Fraction-free Gaussian elimination keeps intermediate values the same size as Cramer's, so it doesn't change what fits. Check that every division is exact: a remainder means bad hailstones or a mistake.

## Don't simulate what you can compute

- **Huge repetition counts** (2023 d14, a billion cycles): find where the state repeats and jump ahead with `%`. Save each step's answer as you go, so you don't have to replay the cycles to reach it.
- **Expanding the input** (2023 d11, a million times wider): don't build the bigger grid; compute each position's offset (the empty rows and columns before it).
- **Area of a dug-out loop** (2023 d18): don't dig into a grid and flood-fill; the shoelace formula on the corner points gives the area, and Pick's theorem adds the trench itself: `area + perimeter / 2 + 1`. The `+ 1` is the loop's full turn: a closed loop has 4 more outer corners than inner ones, each adding an extra quarter square. With distances in the hundreds of thousands (part 2), it's the only way.
- **Line of sight on a grid** (2019 d10): don't walk from one point toward another looking for blockers. Two points hide each other exactly when they lie in the same direction, and the direction is the offset divided by its `gcd`. "How many are visible" is the number of distinct directions: `.map(direction(pos, it)).to_set().len()`, exact with integers, no floats or angles. Walking is right when you need what's *on* the path, not just whether it's clear: in part 2, the rotation that hits an asteroid is the number of asteroids between it and the station, counted over `1..gcd` steps. The whole firing order is then one sort by `(rotation, angle)`, with no simulated laser and no state.
- **Sorting by angle without floats** (2019 d10 part 2): a quadrant offset plus `y / (x + y)` within the quadrant grows with the angle, so it works as an integer sort key. But integer division rounds, and two directions can get the same key: with `mask = rows * cols`, the 3,072 directions of a 36×36 grid got only 2,700 keys. Two different fractions with denominators up to `D` differ by at least `1 / D²`, so scaling by `D²` (`pow(rows + cols, 2)`) keeps every key distinct.
- **Counting a range that has a boundary** (2023 d6): when the condition holds on one interval, binary search for its edge instead of testing every value.

## Check whether the input is nicer than the puzzle promises

Sometimes the general problem is hard but the actual input has extra structure. In 2023 d8 each start runs in a clean cycle, so the answer is the `lcm` of the cycle lengths, which the puzzle text never promises. Look at the input (print the cycle lengths, the first few states) before writing the general solution. Then check the shortcut's own precondition on what you printed: in 2023 d20, `rx` is fed by one Conjunction whose inputs each send a high pulse on a regular cycle, and `lcm` is right only because each input's first high pulse comes exactly at its period (no offset). Write that assumption in a comment next to the `lcm`.

## Beware an input that passes by luck

A wrong algorithm can still give the right answer on your input. In 2023 d10 part 1, the search followed one-sided pipe connections and wandered into about 3,300 cells off the loop. The answer was right only because none of those branches got farther than the loop's farthest point. The puzzle's noisy example caught it. In 2023 d17 part 2, letting the crucible stop at the end before its minimum straight run still gave the right input answer, because the cheapest arrival there happened to be legal; the puzzle's second example (47 instead of 71) caught it. Run every example in the puzzle text, not just the clean one. In 2023 d19 part 2, a rule whose value lies outside the current range would produce a piece with a negative size and grow the other piece past the range's ends; the input never contains such a rule, so the answer is right anyway. In 2023 d20 part 2, recording the press number on *every* high pulse overwrote a period with a later multiple whenever an input fired twice before the others had fired once; the input passed only because its periods are close together, and recording only the first hit fixed it. Relying on that is fine once you know it (see the section above); the danger is relying on it without knowing.

## A total that adds up doesn't prove the parts are right

A control sum, like checking that all the pieces of a split add up to the whole (2023 d19: every accepted and rejected range together is `pow(4000, 4)`), catches pieces that overlap or go missing. It can't catch a mistake that moves weight from one piece to another: a split one off moves values between the two sides, and the total stays the same. Test the pieces themselves on a small case you can count by hand.
