Solving AoC 2023 day 23 part 1

```egg
let grid = input.to_char_grid()
# start is the only . on the first line
let start = grid.find('.')
# finish is the only . on the last line
let finish = grid.find_last('.')
let seen = Set()

fun dfs(pos: (int, int), dist: int) -> int:
  if pos == finish:
    return dist
  if grid[pos] == '#' or !seen.add(pos):
    return 0

  let next = []
  if grid[pos] == '.':
    next = grid.neighbors4(pos).to_list()
  else:
    next = [pos + grid[pos].dir()]

  let max = next.map(n => dfs(n, dist + 1)).max()

  seen.remove(pos)
  max

dfs(start, 0)
```

A neater solution for part 1: the slopes make the maze acyclic, so the longest path can be memoized instead of searched with backtracking. It relies on that property of the input, so it doesn't work for part 2, where slopes become normal paths and cycles appear.

```egg
let grid = input.to_char_grid()
let start = grid.find('.')
let finish = grid.find_last('.')

# The slopes make the maze acyclic: no seen set, just never step back.
memo fun longest(pos: (int, int), prev: (int, int)) -> int:
  if pos == finish:
    return 0
  1 + grid.neighbors4(pos)
    .filter(n => n != prev and grid[n] != '#')
    .filter(n => grid[n] == '.' or grid[n].dir() == n - pos)
    .map(n => longest(n, pos))
    .max()

longest(start, start)
```
