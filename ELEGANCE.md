# Writing elegant AoC solutions

A framework drawn from reviews of the solutions in `aoc-implementation/`. Choosing the right algorithm is in `LEARNINGS.md`. Each rule points back to a day where it came up. New rules are added as reviews find them.

## 1. Separate deciding from doing

When a loop works out what to do and also does it, move the deciding into a small function that returns data, and keep the loop for the doing.

- **2023 d10:** `exits(pipe)` returns directions. The search only follows them.
- **2023 d16:** `bounce(pos, dir)` returns directions, so the loop body is one line.
- **2023 d18:** the `match` on `"U"`/`"R"`/… returns the direction, and a single `push` uses it, instead of four arms that each push.

A sign you need this: the same action, like `queue.push(...)`, is written in many branches.

## 2. Write each case once and pass in what differs

If you copy a block and change one thing, that one thing is a parameter.

- **2023 d14:** four tilts became one `tilt(dir)`.
- **2023 d11:** parts 1 and 2 differ only by a factor (`1` and `999999`).
- **2023 d10 part 2:** the eight-case `match` collapsed into reusing `exits`.

## 3. Reach for a built-in before writing a loop

Before writing `for`, `break` and a counter, ask whether a chain method already says it:

| You wrote | It's already |
|---|---|
| loop with `break` and `+= 1` | `.filter(it < r).count()` (2023 d11) |
| `enumerate().filter().map().first()` | `.position(...)` (2023 d15) |
| slicing out one element | `.filter(it != x).to_list()` (2023 d15) |
| `.to_sorted()` just to make a list | `.to_list()` (2023 d11) |
| a second pass to recompute | save results as you go (2023 d14 `loads`) |
| `ans = max(ans, …)` in a loop | a function per case, then `.map(f(it)).max()` (2023 d16) |

## 4. Say what happens, not how

Code should read like the puzzle's sentence.

- "Remove the lens with this label" is `filter(l != label)`, not index slicing (2023 d15).
- "A rock moves up while the cell above is empty" is the step-by-step `while`, not computing the destination first (2023 d14).

- "Can't reverse, at most 10 straight, at least 4 before turning or stopping" is one condition per rule inside a single move loop, not a separate branch for the first 4 blocks with its own `push` (2023 d17 part 2).

When the code reads like the puzzle text, you can check it against the puzzle line by line.

## 5. Name things after the puzzle

Use the puzzle's own words: `predict` and `extrapolate` (2023 d9), `current_value`, `boxes` (2023 d15), `bounce` and `energized` (2023 d16). A reader holding the puzzle text then needs no glossary.

## 6. Destructure instead of indexing

`for (i, (r1, c1)) in stars.enumerate():` beats `stars[i]` followed by a `let` (2023 d11). Names at the point of use are clearer than `[0]` and `[i]`, and indexes are where off-by-ones live.

## 7. Prefer code where order doesn't matter

If swapping two lines would break the program, either restructure so it can't break, or add a comment.

- **2023 d14:** moving one step at a time made the two assignments safe in either order.
- **2023 d10 part 2:** a single scan direction makes `inside` reset itself, so no reset is needed.

## 8. Delete what doesn't do anything

After it works, read every line and ask what breaks without it:

- an unused `i` from `enumerate` (2023 d12)
- a `memo` that never gets a cache hit (2023 d12)
- `period`, `offset` and an extra loop (2023 d14)
- the `loop` set, once the grid could record the loop itself (2023 d10)

## 9. Let the data structure do the bookkeeping

Mark state in the grid you already have instead of a parallel `Set`. In 2023 d10 part 2, writing `'O'` into `grid2x` replaced two sets, and the answer became "cells still `'.'`".

## 10. Tests are part of the solution

- Add every example from the puzzle text, especially the noisy or tricky ones. 2023 d10's noisy example caught a real bug that the input hid.
- When a part has no direct example, check it another way (2023 d11's factor 10 and 100).

## 11. Algorithm first, micro-optimizations rarely

When it's slow:
1. Measure where the time goes.
2. Ask whether the growth is the problem (exponential, 2023 d12) or the constant factor (interpreter speed, 2023 d14).

Only an algorithm change is worth losing readability. A 10% win isn't: in 2023 d14, the sweep tilt was only 7% faster and brought the assignment-order trap back. And measure the "algorithm change" too: in 2023 d17, A* added a function and a second number to every queue entry, and ran slower than plain Dijkstra, because its estimate (one heat loss per remaining step) was far below the real cost and pruned almost nothing.

## 12. Describe a set of cases by its rule, not by listing it

When cases come in several hand-written groups, look for the one condition they all share and filter for it. The rule covers the corners and edge cases for free, and it can't miss a group.

- **2023 d16 part 2:** four loops listing the start beams for each edge became "every position and direction where the beam enters from outside": `grid.positions().flat_map(pos => Dirs4.map((pos, it))).filter((pos, dir) => pos - dir !in grid)`. Corners get both of their starts without special handling.

## The routine

After the answer is accepted:

1. **Repetition:** find copy-pasted blocks and turn them into a function or a parameter (rules 1 and 2).
2. **Loops:** check each one against the built-ins (rule 3).
3. **Read aloud:** compare each line to the puzzle sentence it implements (rules 4 and 5).
4. **Delete:** remove what does nothing (rule 8).
5. **Tests:** make sure every puzzle example is there (rule 10).

Each pass takes a minute or two, and doing it every day is what builds the habit.
