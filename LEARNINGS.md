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

## Don't simulate what you can compute

- **Huge repetition counts** (2023 d14, a billion cycles): find where the state repeats and jump ahead with `%`. Save each step's answer as you go, so you don't have to replay the cycles to reach it.
- **Expanding the input** (2023 d11, a million times wider): don't build the bigger grid; compute each position's offset (the empty rows and columns before it).
- **Counting a range that has a boundary** (2023 d6): when the condition holds on one interval, binary search for its edge instead of testing every value.

## Check whether the input is nicer than the puzzle promises

Sometimes the general problem is hard but the actual input has extra structure. In 2023 d8 each start runs in a clean cycle, so the answer is the `lcm` of the cycle lengths, which the puzzle text never promises. Look at the input (print the cycle lengths, the first few states) before writing the general solution.

## Beware an input that passes by luck

A wrong algorithm can still give the right answer on your input. In 2023 d10 part 1, the search followed one-sided pipe connections and wandered into about 3,300 cells off the loop. The answer was right only because none of those branches got farther than the loop's farthest point. The puzzle's noisy example caught it. In 2023 d17 part 2, letting the crucible stop at the end before its minimum straight run still gave the right input answer, because the cheapest arrival there happened to be legal; the puzzle's second example (47 instead of 71) caught it. Run every example in the puzzle text, not just the clean one.
