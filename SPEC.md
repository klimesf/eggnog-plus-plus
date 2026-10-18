# eggnog++ — language spec

Agreed decisions only. Open questions live in PLAN.md.

## Types

### Generics

Only the built-in containers take type parameters: `Vec<T>`, `Map<K, V>`, `Set<T>`, `Deque<T>`, `Heap<T>` and the grid type (final names not settled yet). User code cannot declare generic functions or types. The compiler hard-codes the signatures of built-in methods and generates a separate version of each container for every element type used.

### Numbers

- `int` is a signed 64-bit integer and `float` is a 64-bit IEEE float. There are no other numeric types for now.
- There are no implicit numeric conversions. Mixing types needs an explicit `as` (`x as float`). This keeps it cheap to add `i128` (or `u64`, `i32`) later, when a puzzle needs it.
- **Integer overflow always traps**, in every build. That covers `+`, `-`, `*`, unary `-`, and `int::MIN / -1`. Division or remainder by zero also traps. A trap prints an error with the source location and exits with a non-zero status. If benchmarks show the checks matter, they can be limited to debug builds later.
- Floats follow IEEE rules: overflow gives `inf` and invalid operations give `NaN`. They never trap.

### Characters and strings

- `char` is a single byte, meant for ASCII. There is no Unicode support.
- `c.dir()` maps an arrow (`^ > v <`) or a letter (`U R D L`) to `Up`/`Right`/`Down`/`Left`, and traps on any other char.
- A string is a sequence of bytes, so indexing it (`s[i]`) is O(1) and returns a `char`.

### Grid

- `Grid<T>` is a built-in 2D grid stored as one flat, row-major allocation with a width and a height.
- It is indexed by a point: `g[p]`.
- Constructors set the element type: `input.to_char_grid()` → `Grid<char>` (one char per cell), `input.to_digit_grid()` → `Grid<int>` (each cell 0–9).
- Points are `(row, col)` tuples. `+` and `-` work element by element on `(int, int)` tuples (`pos + Up`, `n - pos`), and `*` by an `int` scales them. Row 0 is the top line of the input, so up is `(-1, 0)` and right is `(0, 1)`.
- `p in grid` / `p !in grid` checks bounds. **`g[p]` with `p` outside the grid traps.** Both row and column are checked, so there is no wrap-around into the next row.
- Parsing ignores a trailing newline and accepts `\r\n` line endings.
- Built-in helpers:
  - `neighbors4(p)` / `neighbors8(p)`: in-bounds neighbors in `Dirs4`/`Dirs8` order (lazy)
  - `find(c)` / `find_last(c)`: position of the first/last matching cell in row-major order (a maybe-value, see Missing values)
  - `positions()`: all points in row-major order (lazy)
  - `rows()` / `cols()`: the grid's size
  - `print()`: prints the grid for debugging
  - `Grid(rows, cols, fill)`: a new grid filled with one value
  - rotate/flip: names to be decided
- Assigning a grid copies it (value semantics): `let next = grid`.

## Program structure

- **A program file is a script.** Its top-level statements are the body of an implicit `solve(input)` function.
- `input` is a predefined `string`, read by the runner from a file or stdin.
- The value of the last expression is the answer: an `int`, a `string`, or a pair of those (`(part1, part2)`). A single value means only one part (while hacking on part 1, or on day 25). A top-level `return` ends the program early with an answer.
- Top-level `fun`s that use script variables are local closures (see Local functions). Top-level `fun`s that don't are plain functions that `test` blocks can call.

### Tests (proposed)

```egg
test "example.txt" == (36, 81)     # runs the whole script on that file, with a fresh state
test "input.txt" == (552, 1225)

test "score":                      # a unit test of a plain function
  assert score('a') == 1
```

`eggnog test day10.egg` runs them. Each input test runs the script from scratch, including empty memo caches.

## Built-in constants

- `Dirs4`: the 4 directions in clockwise order: up, right, down, left. So `Dirs4[(i + 1) % 4]` turns right.
- `Dirs8`: the 4 directions plus the diagonals.
- `Up = (-1, 0)`, `Right = (0, 1)`, `Down = (1, 0)`, `Left = (0, -1)`.
- Concretely: `Dirs4 = [Up, Right, Down, Left]`.
- Constants use PascalCase: `Dirs4`, `int::Max`, `int::Min`.
- `Dirs8` is also clockwise from up: `(-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1)`.

## Collections

### Constructors

A container constructor's arguments are the same as one `push`/`add`: `Set(start)`, `Queue(start)`, `MinQueue(start, 0)`, or empty: `Set()`. To build from a collection, use a chain terminal: `[a, b].to_set()`. (Accepting both forms would be ambiguous when the element is itself a list.)

### Map

- `Map()` traps on a missing key: `m[k]`.
- `Map(default = v)` returns `v` for a missing key, so `counts[k] += 1` works. Reading a missing key doesn't insert it.
- `k in m` checks whether a key is present.

### Queue and Stack

- `Queue`: first in, first out. `Stack`: last in, first out. Both have `push(x)`, and `pop()` returns a maybe-value.
- Swapping `Queue` for `Stack` turns the iterative BFS into an iterative DFS.

### MinQueue

- `MinQueue` is a priority queue that always pops the **lowest** priority first. Priorities are `int`s. For a max-queue, push `-priority`.
- `q.push(item, priority)`. `q.pop()` returns `(item, priority)` as a maybe-value: `while (pos, dist) = q.pop():`.
- Items don't need to be comparable, and there is no comparator (it would be an escaping lambda).
- The order between equal priorities is unspecified.
- `MinQueue(item, priority)` creates the queue with one entry, and `MinQueue()` creates an empty one.

## Chains

- `map`, `filter`, `flat_map`, ranges (`0..`) and similar are **lazy**: they process one element at a time and build no intermediate collections. The compiler fuses a chain into nested loops.
- A chain can't be stored (it holds lambdas), so it must be consumed in the expression that builds it: by a terminal (`sum`, `count`, `any`, `min`, `max`, …), by `for x in chain:`, or by an explicit `to_list()` / `to_set()` / `to_map()`. Assigning a bare chain is a compile error.
- Side effects in lambdas happen in element order.
- `min()` / `max()` (and similar terminals with no answer on an empty chain) return a maybe-value: they trap on an empty chain unless used in a conditional binding.

## Naming conventions

The shape of a method's name tells you what it does:

1. **Verbs change the receiver:** `queue.push(x)`, `queue.pop()`, `seen.add(p)`.
2. **`to_` builds a new container** (it allocates): `to_list()`, `to_set()`, `to_map()`, `to_char_grid()`. Every allocation in a program goes through a `to_` call or a constructor.
3. **Nouns are lazy views or computed values,** with no mutation and no allocation: `input.lines()`, `line.ints()`, `grid.neighbors4(p)`, `len()`, `sum()`.
4. **`is_` / `has_` are predicates:** `is_empty()`.

Exceptions: `find`, `filter` and `map` are verbs that don't mutate. They're kept because the names are standard.

## Missing values

There is no user-visible Option type for now. An operation that may have no result (`queue.pop()`, `grid.find(c)`, and later probably `map.get`) can only be used in a conditional binding:

```
while pos = queue.pop():
if p = grid.find('S'):
```

The body runs only when there is a value. Using such an operation outside a condition traps when the value is missing. A "maybe" value can never be stored or passed around. So `let start = grid.find('S')` is valid and traps if there is no `S`.

A conditional binding can destructure tuples: `while (pos, dist) = queue.pop():`.

## Syntax

### Functions

`fun name(a: int, b: (int, int)) -> int:` declares a function. **Parameter and return types are required** on every named function (top-level and local, `memo` or not). If `-> T` is omitted, the function returns nothing. The last expression in the body is its value, and `return` exits early.

Types are inferred everywhere else: `let` variables, lambda parameters (from the method they're passed to), and container element types (from later use, e.g. `let seen = Set()` followed by `seen.add(p)`).

### Named arguments

- Any argument can be passed by name: `f(a, step = 2)`. Positional arguments come first, then named ones in any order. Each parameter can be given only once.
- This works the same for built-ins: `Map(default = 0)`, `Grid(rows, cols, fill = '.')`.
- It's resolved entirely by the type checker (names are reordered into positional order), so it has no runtime cost.
- **Assignment is a statement, never an expression**, so `f(x = 1)` always means a named argument. (`if x = …` / `while x = …` is conditional-binding syntax, not an assignment expression.)
- Parameters can have default values: `fun f(a: int, step: int = 1)`. Defaults must be constant expressions (literals, constants like `int::Max` or `Dirs4[0]`), and the caller fills them in during type checking. Parameters with defaults come after those without.

### Parameters

- Parameters are **read-only** and passed by pointer under the hood, so passing a grid or a map never copies it.
- To change the caller's value, mark the parameter `inout` and the argument `&`, at both places:
  ```egg
  fun count(stone: int, blinks: int, inout cache: Map<(int, int), int>) -> int: ...
  count(s, 75, &cache)
  ```
- A function that wants its own modifiable copy writes `let g = grid`, an explicit copy.
- Assigning to a read-only parameter, or calling a mutating method on it (`push`, `add`, …), is a compile error.

### Local functions

- A `fun` that uses variables from the script (or from an enclosing function) is a **local function**: a named closure that can't escape, under the same rules as lambdas. It captures by reference, can't be stored or passed as a value, and can call itself.
- Plain local functions may change captured variables (e.g. `seen.add(p)` in a DFS helper).

### Memoized functions

```egg
memo fun blink(stone: int, blinks: int) -> int:
  ...
```

- `memo fun` caches results keyed by the tuple of arguments. The compiler creates a hidden `Map` per function that is checked on entry and filled on return.
- Parameters must be hashable values (ints, chars, strings, tuples of those). `inout` parameters are not allowed.
- A `memo fun` that captures nothing has a global cache that lives until the program exits.
- A `memo fun` that captures variables has a cache per run of its enclosing script or function, so separate test runs never share answers.
- The cache key is only the arguments. So **changing a captured variable after the `memo fun` is defined is a compile error** (assigning it, calling a mutating method on it, or passing it as `&`). Without aliasing, the compiler can check this just by reading the code.

### Lambdas

- Syntax: `p => expr`, `(a, b) => expr`, with optional types `(p: int) => expr`.
- **Lambdas cannot escape.** A lambda can only be written directly as an argument to a call. It can't be stored in a variable, returned, or put in a container.
- Because of that, lambdas capture variables by reference (`.filter(n => seen.add(n))` changes the real `seen`). They need no allocation, and built-in methods (`map`, `filter`, `flat_map`, `any`, …) inline them into plain loops.

### Variables

`let x = ...` declares a variable. `let` is a placeholder keyword and may change. All variables are mutable, with no mutability marker.

### Blocks

Blocks are defined by indentation, Python style. A line that opens a block ends with `:`. A short body can go on the same line after the colon (`if done: break`).

**Tab characters in indentation are a lexer error.** Indent with spaces only.

### Operator precedence

From tightest to loosest: unary `-` `!`, then `*` `/` `%`, then `+` `-`, then comparisons and membership (`==` `!=` `<` `<=` `>` `>=` `in` `!in`), then `and`, then `or`. So `pos + Up in grid` means `(pos + Up) in grid`.

### Line continuation

- A line that starts with `.` continues the expression on the previous line, so chains can be spread over several lines.
- Newlines inside `()` and `[]` are ignored, as in Python.

### Comments

`#` starts a comment that runs to the end of the line.
