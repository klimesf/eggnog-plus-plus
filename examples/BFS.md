An example of BFS algorithm on a 2D grid using eggnog++

Note: A program file is a script: its top-level statements are the body of an implicit `solve(input)`. `input` is a predefined `string` that the runner reads from a file or stdin. The value of the last expression is the answer (an `int`, a `string`, or a pair of those), and a top-level `return` ends the program early with an answer.

Note: Dirs4 is a built-in constant, a list of 4 directions ordered clockwise (up, right, down, left). We will also need Dirs8 that will add diagonal directions, too. We only work with 2D grids now.

```egg
let grid = input.to_digit_grid()
let seen = Set()
let queue = Queue((0, 0))
let ans = 0

while pos = queue.pop():
  if pos !in grid or !seen.add(pos):
    continue
  ans += grid[pos]
  for d in Dirs4:
    queue.push(pos + d)

ans
```

An extended example solving shortest distance on a map made from `.` and `#` where `.` is a field where you can step on and `#` is where you cannot. `F` marks the finish, `S` marks the start.

```egg
let grid = input.to_char_grid()
let seen = Set()
let start = grid.find('S')
let queue = Queue((start, 0))

while (pos, dist) = queue.pop():
  if pos !in grid or !seen.add(pos):
    continue
  if grid[pos] == 'F':
    return dist
  if grid[pos] == '#':
    continue
  for d in Dirs4:
    queue.push((pos + d, dist + 1))

panic("Cannot reach F")

test "example.txt" == 22
test "input.txt" == 304
```

A functional-like approach to the algorithm above

Note: `grid.neighbors4(p)` yields in-bounds neighbors only

```egg
let grid = input.to_char_grid()
let start = grid.find('S')
let seen = Set(start)
let frontier = [start]

for dist in 0..:
  if frontier.is_empty():
    panic("Cannot reach F")
  if frontier.any(p => grid[p] == 'F'):
    return dist
  frontier = frontier
    .flat_map(p => grid.neighbors4(p))
    .filter(n => grid[n] != '#' and seen.add(n))
    .to_list()
```
