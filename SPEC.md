# eggnog++ — language spec

Agreed decisions only. Open questions live in PLAN.md.

## Types

### Generics

Only the built-in containers take type parameters: `Set<T>`, `Map<K, V>`, `Queue<T>`, `Stack<T>`, `MinQueue<T>` and `Grid<T>`. User code cannot declare generic functions or types. The compiler hard-codes the signatures of built-in methods and generates a separate version of each container for every element type used.

### Type notation

A type is written the way its values look where there is a literal for them, and as `Name<T>` where there isn't:

- `int`, `float`, `char`, `string`, `bool`
- tuples: `(int, int)`, like the tuple `(1, 2)`
- lists: `[int]`, like the list `[1, 2]`; nested: `[[char]]`
- the other containers: `Set<(int, int)>`, `Map<string, int>`, `Queue<T>`, `Stack<T>`, `MinQueue<T>`, `Grid<char>`
- structs and enums by name: `Monkey`, `[Instr]`, `Map<string, Module>`

Types appear only after `:` in a parameter and after `->`, so `[int]` never clashes with a list expression like `[x] * n`.

### Numbers

- `int` is a signed 64-bit integer and `float` is a 64-bit IEEE float. There are no other numeric types for now.
- There are no implicit numeric conversions. Mixing types needs an explicit `as` (`x as float`). This keeps it cheap to add `i128` (or `u64`, `i32`) later, when a puzzle needs it. `as` converts between `int` and `float` and between `char` and `int` (see Characters), nothing else: a string is parsed with `.int()`.
- `%` is the Euclidean remainder: for `a % m` the result is always in `0..abs(m)`, never negative. So `(c + dc) % width` wraps a coordinate that went below 0 back around (Rust's `rem_euclid`, Python's `%` for a positive `m`): `-1 % 5 == 4`.
- `/` on ints rounds down to match (Rust's `div_euclid`, Python's `//` for a positive divisor): `-7 / 2 == -4`, so `a == (a / b) * b + a % b` always holds. On a grid that repeats forever, `pos / size` is the copy you're in and `pos % size` the place inside it.
- **Bitwise operators** on ints: `&`, `|`, `^` (xor), `<<`, `>>`, and `&=`, `|=`, `^=`, `<<=`, `>>=`. A shift by less than 0 or more than 63 traps, and so does a `<<` that pushes bits off the end (like other overflow). `>>` keeps the sign. They don't work on bools, which have `and`, `or` and `!=`. There is no bitwise not: `x ^ mask` flips the bits inside a known width.
- **Integer overflow always traps**, in every build. That covers `+`, `-`, `*`, unary `-`, and `int::MIN / -1`. Division or remainder by zero also traps. A trap prints an error with the source location and exits with a non-zero status. If benchmarks show the checks matter, they can be limited to debug builds later.
- `min(a, b)` / `max(a, b)` return the smaller/larger of two values of the same type, compared like `<` (ints, chars, strings, tuples, lists): `best = max(best, x)`. For a whole chain use the terminals `xs.min()` / `xs.max()`. `abs(n)` is the absolute value of an `int`, for Manhattan distances: `abs(r1 - r2) + abs(c1 - c2)`. It traps on `int::Min`, like other overflow.
- `pow(base, exp)` raises an `int` to a non-negative `int` power: `pow(2, matches - 1)`. A negative exponent traps, and so does overflow. `pow(base, exp, mod = m)` is modular exponentiation: the result is in `0..m`, and it never overflows, even when `m` is close to `int::Max` (the runtime multiplies in 128 bits). There is no `**` operator.
- `gcd(a, b)` and `lcm(a, b)` are the greatest common divisor and least common multiple of two `int`s, for cycle lengths that line up: `steps = lcm(steps, cycle)`. Both are never negative; `gcd(n, 0) == n` for `n >= 0`, `gcd(0, 0) == 0` and `lcm(0, n) == 0`. `lcm` traps on overflow, like other overflow, and so does `gcd(int::Min, 0)`.
- Floats follow IEEE rules: overflow gives `inf` and invalid operations give `NaN`. They never trap.

### Characters and strings

- `char` is a single byte, meant for ASCII. There is no Unicode support.
- Chars compare by their code (`c >= 'a'`), and `char - char` is the `int` distance between them: `c - 'a' + 1`. That's the only arithmetic on chars: `'a' + 1` is an error.
- `c as int` is a char's code, 0–255: `'a' as int == 97`, for hashes over ASCII codes (2023 d15). `n as char` goes back and traps outside 0–255: `(c as int + 1) as char` is the next letter.
- `a + b` joins two strings, or a string and a char on either side: `"ab" + "cd"`, `s += c`, `c + s`. Adding an int is an error (use `n.to_string()`), and `'a' + 'b'` stays an error, because chars have no `+`. Each `+` builds a new string, so build long strings with `to_string(sep = …)` on a chain rather than `+=` in a long loop.
- Strings compare character by character, by code, and a prefix is smaller: `"ab" < "b"`, `"ab" < "abc"`.
- `s.split(sep)`: a lazy chain of the parts of `s` between occurrences of `sep`: `input.split("\n\n")` gives the blank-line-separated sections.
- `s.int()`: parses the whole string as an integer (optional leading `-`, digits only). Traps on anything else. To pull numbers *out of* text, use `ints()`. `s.int(base = 16)` parses another base, 2–36, with letters in either case: `"70c71".int(base = 16)`, `"1011".int(base = 2)`.
- `s.split_once(sep)` → a pair `(before, after)` of strings. Traps if `sep` doesn't occur.
- `s.len()`, `s.starts_with(prefix, at = 0)` (does `prefix` occur at position `at`?), `s[i]` → `char`, and `s[a..b]` / `s[a..]` → a new `string`. Slicing copies, as everything does under value semantics.
- `s.lines()`: a lazy chain of the lines of `s`, without their `\n` / `\r\n`.
- `s.ints()`: a lazy chain of every integer in `s`, skipping everything else. A `-` counts as a minus sign only when it directly precedes a digit and doesn't follow one: `"x=-2, y=7"` → `-2, 7`, but `"2-4,6-8"` → `2, 4, 6, 8` (ranges like AoC 2022 d4). Out-of-range numbers trap.
- `s.digits()`: a lazy chain of every digit char in `s` as an `int`, skipping all other chars: `"a1b22".digits()` → `1, 2, 2`. It's the single-digit counterpart of `ints()`.
- `n.digits()` on an `int`: its decimal digits, most significant first, ignoring the sign: `1204.digits()` → `1, 2, 0, 4`, and `0.digits()` → `0`. Lazy.
- `n.to_string()`: an int's decimal text (allocates, hence `to_`).
- `c.is_digit()` is true for `'0'`…`'9'`, and `c.is_letter()` for `'a'`…`'z'` and `'A'`…`'Z'`. `c.digit()` turns `'7'` into `7` and traps on any other char.
- Strings can be iterated directly (`for c in s:`). `s.chars()` is the same as a lazy chain, and `s.count(c)` counts occurrences of a char.
- Chain methods (`map`, `filter`, `any`, `intersection`, …) need `s.chars()` first: `hand.chars().map(strength)`. Only `count` and `rev` work on a string directly. This keeps string methods and chain methods apart, so a name never means both. A string can still be the argument where a chain method takes a sequence: `a.chars().intersection(b)`, `xs.zip(s)`.
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
  - `to_string()`: the rows joined with `\n`, the same text `print()` shows
  - `Grid(rows, cols, fill)`: a new grid filled with one value
  - rotate/flip: names to be decided
- Assigning a grid copies it (value semantics): `let next = grid`.

### Structs and enums

```egg
struct Monkey:
  items: [int]
  operation: string
  divisor: int
  inspected: int = 0

enum Instr:
  Noop
  Addx(n: int)
  Jump(offset: int, flag: string)

let m = Monkey([79, 98], "old * 19", divisor = 23)
monkeys[i].items.push(x)
monkeys[i].inspected += 1

match instr:
  Noop: cycle += 1
  Addx(n): x += n
  Jump(offset, _): pc += offset
```

- **Declared at the top level only**, and known everywhere in the file, before and after the declaration. One field or variant per line. Fields have a type and may have a default (`inspected: int = 0`); fields with defaults come last.
- **An enum is a list of variants.** A variant has no fields (`Noop`) or named fields like a struct (`Addx(n: int)`). Each variant can have different fields.
- **Building a value is a call** with the rules of named arguments: positional first, then by name, defaults filled in: `Monkey([79, 98], "old * 19", divisor = 23)`, `Addx(3)`. A variant without fields is just its name: `Noop`, not `Noop()`.
- **Names:** struct, enum and variant names use PascalCase, and variants are used without the enum's name (`Addx(3)`, not `Instr::Addx(3)`). All of these share one namespace with each other, the functions and the built-in constants and types, so `enum Turn: Left, Right` is a compile error (`Left` is the built-in direction), and so is the same variant name in two enums.
- **Struct fields are read with `.`** (`m.divisor`), and a field is a place like an index: `monkeys[i].items.push(x)` and `m.inspected += 1` change the value in its container. A parameter's fields are read-only like the parameter.
- **An enum's fields are read with `match`.** A variant pattern binds the fields by position, `_` skips one, and the number must match the declaration: `Jump(offset, _):`. The bindings are read-only copies; to change the data, assign a whole new value: `lights[i] = On(level - 1)`.
- **A `match` on an enum must handle every variant**, or have a `_` arm; a missing variant is a compile error.
- **Value semantics**, like everything else: assigning a struct or variant copies it.
- **Equality, hashing and ordering come for free**, so they work in `Set`s, as `Map` keys, as `memo fun` parameters and in `to_sorted`. Variants order by their declaration order, then field by field (like Rust's `#[derive]`); two structs order field by field. Fields must themselves be comparable to order.
- **Types can be recursive**: `enum Tree:` with `Leaf(value: int)` and `Node(children: [Tree])`. Value semantics means a value never contains itself, so trees are safe; there are no cycles to worry about.
- **Printing** (`print(x)`, failed tests) shows a value the way it's built: `Monkey(items = [79, 98], operation = "old * 19", divisor = 23, inspected = 0)`, `Addx(n = 3)`, `Noop`.
- No methods, generics or traits: write plain functions, `fun size(t: Tree) -> int`.

## Program structure

- **A program file is a script.** Its top-level statements are the body of an implicit `solve(input)` function.
- `input` is a predefined `string`, read by the runner from a file or stdin.
- **One file per part** (e.g. `day01_part1.egg`, `day01_part2.egg`). Code shared between the parts is copied. Shared helpers should move into the standard library instead (see PLAN.md, stdlib principle).
- The value of the last expression is the answer: an `int` or a `string`. A string answer may span several lines (e.g. a drawn screen), and the runner prints it as is. A top-level `return` ends the program early with an answer.
- `panic(message)` stops the program with an error that shows `message` (any value, turned into a string) and the source location, for states that should never happen: `_: panic("unknown opcode " + op.to_string())`. A search that always finds its answer needs no `panic` after it: if the program ends without an answer, that's already an error.
- Top-level `fun`s that use script variables are local closures (see Local functions). Top-level `fun`s that don't are plain functions that `test` blocks can call.

### Input

The runner strips the single trailing newline (`
` or `
`) from `input`. Every AoC input ends with one.

### Tests

```egg
test "(())" == 0                       # a string literal is the input itself
test file("example.txt") == 36        # file(...) reads the input from a file, relative to the .egg file
test file("input.txt") == 552

test "score":                          # a named block is a unit test of a plain function
  assert score('a') == 1
```

`eggnog test day10.egg` runs them. Each input test runs the script from scratch, including empty memo caches. Inline and file inputs get the same trailing-newline stripping as `input`. (The form `test "name":` with a block is still a proposal.)

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
- `m.keys()` and `m.values()` are lazy chains, and `for (k, v) in m:` goes through the entries as pairs. **The order is unspecified** and may differ between runs, so an answer must never depend on it; sort when you need one: `m.keys().to_sorted()`.
- `m.remove(k)` removes a key and returns whether it was there; a missing key is not an error. `set.remove(x)` works the same way.

### Lists

- `[a, b, c]` literals, and `[]` for an empty list (the element type is inferred).
- `xs.sort(by = f, desc = false)` sorts a list **in place** (a verb that changes the receiver). Use `to_sorted` on chains or to keep the original.
- Lists compare element by element, like tuples, and a prefix is smaller: `[1, 2] < [1, 3]`, `[2] > [1, 9]`, `[1] < [1, 0]`. That covers `<`, `min`/`max` and sort keys: `paths.to_sorted(by = (it.len(), it))`. The elements must be comparable themselves (ints, chars, strings, tuples, lists). Bools, sets, maps, queues and grids don't compare.
- `xs.push(x)`, `xs[i]`, `xs.len()`. `xs.first()` / `xs.last()` are maybe-values, so they trap on an empty list.
- `[x] * n` is a list of `n` copies of `x`. Under value semantics the copies are independent: `[Stack()] * 3` is three separate stacks (unlike Python's `[[]] * 3`).
- Slicing: `xs[a..b]`, `xs[a..]`, `xs[..b]` → a new list (a copy).
- `xs.rev()` is a lazy chain in reverse order. `rev()` exists only on lists, strings and ranges, not on other chains.
- **Indexing gives a place, not a copy.** `xs[i].push(x)`, `xs[i] += 1` and `grid[p] = '#'` change the element inside the container. The same goes for nested places like `xs[i][j]` and `m[k].push(x)`.
- Destructuring: `let [a, b, c] = xs` binds the elements of a list and traps if the length doesn't match. Typical use: `let [n, from, to] = line.ints().to_list()`.

### Queue and Stack

- `Queue`: first in, first out. `Stack`: last in, first out. Both have `push(x)`, and `pop()` returns a maybe-value.
- `stack.top()` returns the top element without removing it (a maybe-value).
- `stack.pop_many(n)` removes the top `n` elements and returns them as a list in stack order (bottom to top). It traps if the stack has fewer than `n`.
- `stack.push_all(xs)` pushes every element of a list or chain, in order. So `a.push_all(b.pop_many(n))` moves a block and keeps its order, and `.rev()` in between reverses it.
- Swapping `Queue` for `Stack` turns the iterative BFS into an iterative DFS.

### MinQueue

- `MinQueue` is a priority queue that always pops the **lowest** priority first. Priorities are `int`s. For a max-queue, push `-priority`.
- `q.push(item, priority)`. `q.pop()` returns `(item, priority)` as a maybe-value: `while (pos, dist) = q.pop():`.
- Items don't need to be comparable, and there is no comparator (it would be an escaping lambda).
- The order between equal priorities is unspecified.
- `MinQueue(item, priority)` creates the queue with one entry, and `MinQueue()` creates an empty one.

## Chains

- `map`, `filter`, `flat_map`, ranges (`0..`) and similar are **lazy**: they process one element at a time and build no intermediate collections. The compiler fuses a chain into nested loops.
- A chain can't be stored (it holds lambdas), so it must be consumed in the expression that builds it: by a terminal (`sum`, `product`, `count`, `any`, `min`, `max`, …), by `for x in chain:`, or by an explicit `to_list()` / `to_set()` / `to_map()`, or `to_string(sep = "")`, which joins a chain of chars or strings into one string: `rows.to_string(sep = "\n")`. Assigning a bare chain is a compile error.
- Side effects in lambdas happen in element order.
- Ranges: `a..b` excludes `b`, `a..=b` includes it, `a..` never ends. Ranges are lazy chains.
- `scan(init, f)` yields each running value of `acc = f(acc, x)`, starting from `acc = init`. `init` itself is not yielded: `[1, 1, -1].scan(0, (a, x) => a + x)` gives `1, 2, 1`.
- `sum()` adds the elements (0 for an empty chain). `product()` multiplies them (1 for an empty chain): `Dirs4.map(viewing_distance(pos, it)).product()`. Both trap on overflow.
- `first()` / `last()` return the first/last element of a chain. Both are maybe-values, and `last()` consumes the whole chain.
- `find(pred)` / `find_last(pred)` return the first/last element that matches. Both are maybe-values. `find_last` consumes the whole chain.
- `position(pred)` returns the 0-based index of the first element that matches. It's a maybe-value: it traps if nothing matches, unless used in a conditional binding.
- `enumerate(start = 0)` pairs each element with its index: `for (i, c) in input.chars().enumerate(1):`.
- `a.zip(b)` pairs the elements of `a` and `b` in order, as `(x, y)` tuples: `times.ints().zip(distances.ints()).map((t, d) => ways(t, d))`. It's a lazy step, and `b` is a chain, list, range or string. **The lengths must match:** a mismatch traps, since in AoC it almost always means a parsing bug (like list destructuring). So `b` can't be endless (`0..`); use `enumerate()` to number elements. Only two sequences: there is no `zip3`.
- `min()` / `max()` (and similar terminals with no answer on an empty chain) return a maybe-value: they trap on an empty chain unless used in a conditional binding.
- `max(count = n)` / `min(count = n)` return a list of the `n` largest/smallest elements, largest/smallest first. They trap if the chain has fewer than `n`.
- `take(n)` / `skip(n)` are lazy steps: the first `n` elements / everything after the first `n`. `take` stops pulling as soon as it has `n`, so `(0..).take(3)` ends.
- `chunks(n)` groups the elements into lists of `n`, one after the other: `lines.chunks(3)` gives lines 1–3, then 4–6. The last list is shorter if the elements don't divide evenly. Lazy.
- `windows(n)` gives every run of `n` consecutive elements as a list, overlapping: `[1, 2, 3, 4].windows(3)` gives `[1, 2, 3], [2, 3, 4]`. Nothing if there are fewer than `n`. Lazy.
- `permutations()` gives every ordering of the elements, as lists: `(0..5).permutations().map(amplify(program, it)).max()` (2019 d7; also routes through every city in 2015 d9). `combinations(k)` gives every choice of `k` elements, as lists in their original order: `(1..=4).combinations(2)` gives `[1, 2], [1, 3], …, [3, 4]` (every pair in 2023 d24, containers in 2015 d17). Both first read the whole input (it can't be endless), then yield lazily. Elements are picked by position, in lexicographic order of the positions, as in Python's `itertools`, so equal elements give repeated results; add `.to_set()` to drop them. `combinations(k)` with `k` larger than the count gives nothing, `combinations(0)` gives one empty list, and a negative `k` traps. There are n! permutations, so about 10 elements is the practical limit (3.6 million lists).
- `a.intersection(b)` keeps the elements of `a` that are `in b`. It's a lazy step (a noun, so it doesn't allocate): the order and duplicates of `a` are kept, and `.to_set()` makes a set of it. `b` is a string, list or `Set`. The cost is |a| × (cost of `in b`): O(|a|) for a Set, O(|a| · |b|) for a list. For a string, the compiler can first mark `b`'s chars in a 256-entry table on the stack, giving O(|a| + |b|). Typical use: `a.chars().intersection(b).intersection(c).first()` for the one char three strings share.
- `to_sorted(by = f, desc = false)` returns a new sorted list (it allocates, hence `to_`). `by` gives the sort key, and `desc = true` sorts largest first. The sort is stable. Tuples, lists and strings compare element by element.

## Not included (for now)

- **Regular expressions.** `ints()`, `split_once` and list destructuring cover AoC's input formats. Revisit if a puzzle needs real pattern matching.

## Naming conventions

The shape of a method's name tells you what it does:

1. **Verbs change the receiver:** `queue.push(x)`, `queue.pop()`, `seen.add(p)`.
2. **`to_` builds a new container** (it allocates): `to_list()`, `to_set()`, `to_map()`, `to_char_grid()`. Every allocation in a program goes through a `to_` call or a constructor.
3. **Nouns are lazy views or computed values,** with no mutation and no allocation: `input.lines()`, `line.ints()`, `grid.neighbors4(p)`, `len()`, `sum()`.
4. **`is_` / `has_` are predicates:** `is_empty()`.
5. **PascalCase is for types, variants and constants:** `Grid`, `Monkey`, `Addx`, `Dirs4`. Everything else is snake_case.

Exceptions: `find`, `filter`, `map` and `split` are verbs that don't mutate. They're kept because the names are standard. And a mutating verb may return the elements it removed as a new list (`pop_many`): that allocation doesn't go through `to_`.

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

- Syntax: `p => expr`, `(a, b) => expr`, with optional types `(p: int) => expr`. A single parameter can destructure a list: `.map([a, b, c] => …)` (traps if the length doesn't match, like `let [a, b, c] = xs`).
- **Implicit parameter `it`:** an argument of a lambda-taking method that uses `it` is a one-parameter lambda, so `.map(it.int())` means `.map(x => x.int())`, and `.map(arrangements(it))` calls a function on each element. The lambda-taking methods are `map`, `filter`, `flat_map`, `scan`, `any`, `all`, `count`, `find`, `find_last`, `position`, and the `by =` of `sort` / `to_sorted`. `count` takes a predicate or a value to count: `xs.count(it > 4)`, `xs.count(1)`.
  - `it` belongs to the **innermost** lambda-taking call: in `.filter(it.chars().any(it == 'a'))` the second `it` is a char. To reach an outer element from inside, name it: `.filter(s => s.chars().any(c => c == s[0]))`.
  - Use explicit `x =>` for two or more parameters and for destructuring (`[a, b] => …`).
  - `it` is reserved: it can't be declared as a variable, parameter or lambda parameter, and using it outside such an argument is a compile error.
- **Lambdas cannot escape.** A lambda can only be written directly as an argument to a call. It can't be stored in a variable, returned, or put in a container.
- Because of that, lambdas capture variables by reference (`.filter(n => seen.add(n))` changes the real `seen`). They need no allocation, and built-in methods (`map`, `filter`, `flat_map`, `any`, …) inline them into plain loops.

### Variables

`let x = ...` declares a variable. `let` is a placeholder keyword and may change. All variables are mutable, with no mutability marker.

`let` can declare a name that already exists in the same scope. The new variable replaces the old one from that line on and can have a different type, as in Rust. Typical use: the raw text and its parsed form share a name, `let (workflows, parts) = input.split_once("\n\n")` followed by `let workflows = workflows.lines()…to_map()`. When the parsing needs a loop, put it in a function: `let workflows = parse_workflows(workflows)`.

A `let` inside a block can't reuse the name of a variable from an enclosing block of the same script, function or lambda body; it's a compile error. Since all variables are mutable, `let total = …` inside a loop is almost always a mistake for `total = …`, which would otherwise leave the outer `total` silently unchanged. Only `let` is affected: `for` variables, conditional bindings (`if x = …:`) and lambda parameters may reuse outer names (`energized.map((pos, _) => pos)`). A function or lambda body starts fresh, so a function's local `grid` may share its name with a script-level `grid`.

### Blocks

Blocks are defined by indentation, Python style. A line that opens a block ends with `:`. A short body can go on the same line after the colon (`if done: break`).

**Tab characters in indentation are a lexer error.** Indent with spaces only.

Source files are ASCII. Any other byte outside a string literal is a lexer error. This catches non-breaking spaces (macOS Option+Space), which look like normal spaces.

### Operator precedence

From tightest to loosest: unary `-` `!`, then `as`, then `*` `/` `%`, then `+` `-`, then `<<` `>>`, then `&`, then `^`, then `|`, then ranges (`..` `..=`), then comparisons and membership (`==` `!=` `<` `<=` `>` `>=` `in` `!in`), then `and`, then `or`. So `pos + Up in grid` means `(pos + Up) in grid`, `n + c as int * 2` means `n + ((c as int) * 2)`, and `mask & 1 << k != 0` means `(mask & (1 << k)) != 0` (as in Rust; C and Python would compare first).

### if / else if / else

```egg
if a:
  ...
else if b:
  ...
else:
  ...
```

There is no `elif` keyword: `else if` is `else` followed by an `if`.

### If-expressions

`if cond then a else b` is an expression: `floor += if c == '(' then 1 else -1`. `else` is required, and both branches must have the same type. The block form `if cond:` is a statement.

### match

```egg
match c:
  '(': floor += 1
  ')': floor -= 1
  _: panic("unknown char")
```

- Arms are `pattern: body`. The body goes on the same line or in an indented block. Patterns for now: literals, constants and variants without fields (`Up:`, `Noop:`), variant patterns that bind fields (`Addx(n):`, see Structs and enums), and `_`.
- The first matching arm runs, with no fallthrough. `break`/`continue` inside an arm apply to the enclosing loop. If no arm matches, the program traps.
- `match` can also be an expression when every arm is a single expression: `let step = match c:` followed by arms like `'(': 1`.
- Guards, nested variant patterns and tuple patterns are not designed yet.

### Line continuation

- A line that starts with `.` continues the expression on the previous line, so chains can be spread over several lines.
- Newlines inside `()` and `[]` are ignored, as in Python.

### Multi-line strings

```egg
test """
  1abc2
  pqr3stu8vwx
  """ == 142
```

Triple-quoted strings can span lines (Swift's rule): the newline right after the opening `"""` and the one before the closing `"""` are dropped, and every line is dedented by the indentation of the closing `"""`. Inside the string, the lexer does no INDENT/DEDENT processing.

### Comments

`#` starts a comment that runs to the end of the line.
